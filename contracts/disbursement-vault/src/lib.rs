#![no_std]
//! A Soroban disbursement vault.
//!
//! An admin funds the vault with a single token, registers recipients with
//! fixed allocations, and each recipient claims their own. After a deadline the
//! admin may cancel unclaimed allocations and reclaim the funds.
//!
//! See `docs/DESIGN.md` at the repository root for the full design rationale.

mod error;
mod events;
mod storage;
mod types;

use soroban_sdk::{contract, contractimpl, token, Address, Env};

pub use crate::error::Error;
pub use crate::types::{Allocation, AllocationStatus, Config, DataKey};

/// A vault that holds one token and disburses it to registered recipients.
///
/// The safety invariant, upheld by every entry point, is that the sum of all
/// `Pending` allocations never exceeds the vault's token balance. Because
/// `withdraw` is capped at the *unencumbered* balance, the admin can never pull
/// out funds owed to a recipient who has not yet claimed.
#[contract]
pub struct DisbursementVault;

/// `a + b`, as a typed error rather than a panic.
fn checked_add(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_add(b).ok_or(Error::MathOverflow)
}

/// `a - b`, as a typed error rather than a panic.
fn checked_sub(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_sub(b).ok_or(Error::MathOverflow)
}

#[contractimpl]
impl DisbursementVault {
    /// Configures the vault. Callable exactly once.
    ///
    /// `deadline` is unix seconds and must be in the future: a deadline in the
    /// past would create a vault in which nothing could ever be registered or
    /// claimed.
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        deadline: u64,
    ) -> Result<(), Error> {
        if storage::has_config(&env) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();

        if deadline <= env.ledger().timestamp() {
            return Err(Error::InvalidDeadline);
        }

        storage::set_config(
            &env,
            &Config {
                admin: admin.clone(),
                token: token.clone(),
                deadline,
            },
        );
        storage::set_total_allocated(&env, 0);
        storage::set_total_claimed(&env, 0);
        storage::extend_instance(&env);

        events::Initialized {
            admin,
            token,
            deadline,
        }
        .publish(&env);

        Ok(())
    }

    /// Moves `amount` of the vault token from the admin into the vault.
    ///
    /// Permitted after the deadline too: late funds are simply unencumbered and
    /// the admin can withdraw them again.
    pub fn fund(env: Env, amount: i128) -> Result<(), Error> {
        let config = storage::config(&env)?;
        config.admin.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let client = token::TokenClient::new(&env, &config.token);
        let vault = env.current_contract_address();
        client.transfer(&config.admin, &vault, &amount);

        storage::extend_instance(&env);

        events::Funded {
            from: config.admin,
            amount,
            balance: client.balance(&vault),
        }
        .publish(&env);

        Ok(())
    }

    /// Earmarks `amount` for `recipient`.
    ///
    /// Rejected unless the new total encumbrance still fits inside the vault's
    /// actual balance, which is what makes every pending allocation backed by
    /// real tokens. Also rejected for an address that already has an allocation,
    /// so an allocation can never silently change under a recipient.
    pub fn register(env: Env, recipient: Address, amount: i128) -> Result<(), Error> {
        let config = storage::config(&env)?;
        config.admin.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if env.ledger().timestamp() >= config.deadline {
            return Err(Error::DeadlinePassed);
        }
        if storage::allocation(&env, &recipient).is_some() {
            return Err(Error::AllocationExists);
        }

        let balance =
            token::TokenClient::new(&env, &config.token).balance(&env.current_contract_address());
        let total_allocated = checked_add(storage::total_allocated(&env), amount)?;
        if total_allocated > balance {
            return Err(Error::InsufficientFunds);
        }

        storage::set_allocation(
            &env,
            &recipient,
            &Allocation {
                amount,
                status: AllocationStatus::Pending,
            },
        );
        storage::set_total_allocated(&env, total_allocated);
        storage::extend_allocation(&env, &recipient);
        storage::extend_instance(&env);

        events::Registered {
            recipient,
            amount,
            total_allocated,
        }
        .publish(&env);

        Ok(())
    }

    /// Pays a recipient their own allocation.
    ///
    /// Only valid strictly before the deadline. That exclusive bound, paired
    /// with `cancel`'s inclusive one, means exactly one of claim and cancel is
    /// ever valid — there is no ledger in which they race.
    pub fn claim(env: Env, recipient: Address) -> Result<(), Error> {
        let config = storage::config(&env)?;
        recipient.require_auth();

        if env.ledger().timestamp() >= config.deadline {
            return Err(Error::DeadlinePassed);
        }

        let mut allocation =
            storage::allocation(&env, &recipient).ok_or(Error::AllocationNotFound)?;
        match allocation.status {
            AllocationStatus::Claimed => return Err(Error::AlreadyClaimed),
            AllocationStatus::Cancelled => return Err(Error::AllocationCancelled),
            AllocationStatus::Pending => {}
        }

        let amount = allocation.amount;
        let total_allocated = checked_sub(storage::total_allocated(&env), amount)?;
        let total_claimed = checked_add(storage::total_claimed(&env), amount)?;

        // State is settled before the transfer, so a re-entrant call would
        // already see the allocation as claimed.
        allocation.status = AllocationStatus::Claimed;
        storage::set_allocation(&env, &recipient, &allocation);
        storage::set_total_allocated(&env, total_allocated);
        storage::set_total_claimed(&env, total_claimed);

        token::TokenClient::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &recipient,
            &amount,
        );

        storage::extend_allocation(&env, &recipient);
        storage::extend_instance(&env);

        events::Claimed { recipient, amount }.publish(&env);

        Ok(())
    }

    /// Releases an unclaimed allocation after the deadline.
    ///
    /// Moves no tokens; it only drops the encumbrance so that `withdraw` can
    /// reclaim the funds. Keeping the transfer out means `cancel` cannot fail on
    /// a token error, and the admin can cancel many allocations then withdraw
    /// once.
    pub fn cancel(env: Env, recipient: Address) -> Result<(), Error> {
        let config = storage::config(&env)?;
        config.admin.require_auth();

        if env.ledger().timestamp() < config.deadline {
            return Err(Error::DeadlineNotReached);
        }

        let mut allocation =
            storage::allocation(&env, &recipient).ok_or(Error::AllocationNotFound)?;
        match allocation.status {
            AllocationStatus::Claimed => return Err(Error::AlreadyClaimed),
            AllocationStatus::Cancelled => return Err(Error::AllocationCancelled),
            AllocationStatus::Pending => {}
        }

        let amount = allocation.amount;
        let total_allocated = checked_sub(storage::total_allocated(&env), amount)?;

        allocation.status = AllocationStatus::Cancelled;
        storage::set_allocation(&env, &recipient, &allocation);
        storage::set_total_allocated(&env, total_allocated);
        storage::extend_allocation(&env, &recipient);
        storage::extend_instance(&env);

        events::Cancelled { recipient, amount }.publish(&env);

        Ok(())
    }

    /// Sends unencumbered funds out of the vault.
    ///
    /// Capped at `balance - total_allocated`, so this can never touch tokens
    /// owed to a pending allocation. This cap is the contract's central safety
    /// property.
    pub fn withdraw(env: Env, to: Address, amount: i128) -> Result<(), Error> {
        let config = storage::config(&env)?;
        config.admin.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let client = token::TokenClient::new(&env, &config.token);
        let vault = env.current_contract_address();
        let available = checked_sub(client.balance(&vault), storage::total_allocated(&env))?;
        if amount > available {
            return Err(Error::InsufficientFunds);
        }

        client.transfer(&vault, &to, &amount);
        storage::extend_instance(&env);

        events::Withdrawn { to, amount }.publish(&env);

        Ok(())
    }

    // --- read-only ---------------------------------------------------------
    //
    // Getters deliberately do not extend any TTL: extension is a ledger write,
    // and a third-party read should not subsidise another entry's rent.

    pub fn get_admin(env: Env) -> Result<Address, Error> {
        Ok(storage::config(&env)?.admin)
    }

    pub fn get_token(env: Env) -> Result<Address, Error> {
        Ok(storage::config(&env)?.token)
    }

    pub fn get_deadline(env: Env) -> Result<u64, Error> {
        Ok(storage::config(&env)?.deadline)
    }

    pub fn get_allocation(env: Env, recipient: Address) -> Option<Allocation> {
        storage::allocation(&env, &recipient)
    }

    pub fn total_allocated(env: Env) -> i128 {
        storage::total_allocated(&env)
    }

    pub fn total_claimed(env: Env) -> i128 {
        storage::total_claimed(&env)
    }

    /// Balance not encumbered by any pending allocation: the most the admin
    /// could currently `withdraw`.
    pub fn available(env: Env) -> Result<i128, Error> {
        let config = storage::config(&env)?;
        let balance =
            token::TokenClient::new(&env, &config.token).balance(&env.current_contract_address());
        checked_sub(balance, storage::total_allocated(&env))
    }
}

mod test;
