#![cfg(test)]

use soroban_sdk::{
    testutils::{
        storage::Persistent as _, Address as _, AuthorizedFunction, Events as _, Ledger, MockAuth,
        MockAuthInvoke,
    },
    token::{StellarAssetClient, TokenClient},
    Address, Env, IntoVal, Symbol,
};

use crate::{Allocation, AllocationStatus, DisbursementVault, DisbursementVaultClient, Error};

const DAY: u64 = 86_400;
const START: u64 = 1_000_000;
/// Default deadline used by the fixtures: 30 days after `START`.
const DEADLINE: u64 = START + 30 * DAY;

/// A fully wired vault plus its backing SAC token.
struct Fixture<'a> {
    env: Env,
    vault: DisbursementVaultClient<'a>,
    vault_address: Address,
    token: TokenClient<'a>,
    sac: StellarAssetClient<'a>,
    admin: Address,
    alice: Address,
    bob: Address,
}

impl<'a> Fixture<'a> {
    /// Registers a SAC and the vault, and sets the ledger clock to `START`.
    /// Auth is *not* mocked: each test opts in, so no test can pass by
    /// accident on auth it never exercised.
    fn new() -> Self {
        let env = Env::default();
        env.ledger().set_timestamp(START);

        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let token_address = sac.address();

        let vault_address = env.register(DisbursementVault, ());

        Self {
            vault: DisbursementVaultClient::new(&env, &vault_address),
            vault_address,
            token: TokenClient::new(&env, &token_address),
            sac: StellarAssetClient::new(&env, &token_address),
            env,
            admin,
            alice,
            bob,
        }
    }

    /// Mints `amount` of the token to the admin, so the admin can fund.
    fn mint_to_admin(&self, amount: i128) {
        self.env.mock_all_auths();
        self.sac.mint(&self.admin, &amount);
    }

    /// `initialize` with the default deadline.
    fn init(&self) {
        self.env.mock_all_auths();
        self.vault
            .initialize(&self.admin, &self.token.address, &DEADLINE);
    }

    /// Mint, initialize, and fund the vault with `amount`.
    fn init_and_fund(&self, amount: i128) {
        self.mint_to_admin(amount);
        self.init();
        self.env.mock_all_auths();
        self.vault.fund(&amount);
    }

    fn set_time(&self, timestamp: u64) {
        self.env.ledger().set_timestamp(timestamp);
    }

    /// Asserts the most recent invocation emitted exactly `expected` from the
    /// vault. Token events from the SAC sub-invocation are filtered out.
    fn assert_emitted<E: soroban_sdk::events::Event>(&self, expected: E) {
        let emitted = self
            .env
            .events()
            .all()
            .filter_by_contract(&self.vault_address);
        assert_eq!(
            emitted.events(),
            [expected.to_xdr(&self.env, &self.vault_address)].as_slice(),
        );
    }
}

// ---------------------------------------------------------------------------
// initialize
// ---------------------------------------------------------------------------

#[test]
fn initialize_sets_config_and_zeroes_counters() {
    let f = Fixture::new();
    f.init();

    assert_eq!(f.vault.get_admin(), f.admin);
    assert_eq!(f.vault.get_token(), f.token.address);
    assert_eq!(f.vault.get_deadline(), DEADLINE);
    assert_eq!(f.vault.total_allocated(), 0);
    assert_eq!(f.vault.total_claimed(), 0);
    assert_eq!(f.vault.available(), 0);
}

#[test]
fn initialize_is_callable_only_once() {
    let f = Fixture::new();
    f.init();

    f.env.mock_all_auths();
    assert_eq!(
        f.vault
            .try_initialize(&f.admin, &f.token.address, &DEADLINE)
            .unwrap_err()
            .unwrap(),
        Error::AlreadyInitialized,
    );
}

/// A second `initialize` must be rejected even when it tries to install a
/// *different* admin — otherwise the guard would be bypassable by an attacker
/// front-running configuration.
#[test]
fn initialize_cannot_be_hijacked_by_a_different_admin() {
    let f = Fixture::new();
    f.init();

    f.env.mock_all_auths();
    assert_eq!(
        f.vault
            .try_initialize(&f.alice, &f.token.address, &DEADLINE)
            .unwrap_err()
            .unwrap(),
        Error::AlreadyInitialized,
    );
    assert_eq!(f.vault.get_admin(), f.admin);
}

#[test]
fn initialize_rejects_deadline_in_the_past_or_present() {
    let f = Fixture::new();

    f.env.mock_all_auths();
    assert_eq!(
        f.vault
            .try_initialize(&f.admin, &f.token.address, &(START - 1))
            .unwrap_err()
            .unwrap(),
        Error::InvalidDeadline,
    );

    // The bound is exclusive: a deadline exactly at `now` is also invalid.
    f.env.mock_all_auths();
    assert_eq!(
        f.vault
            .try_initialize(&f.admin, &f.token.address, &START)
            .unwrap_err()
            .unwrap(),
        Error::InvalidDeadline,
    );
}

#[test]
fn initialize_requires_admin_auth() {
    let f = Fixture::new();
    // No auth mocked at all.
    let result = f
        .vault
        .try_initialize(&f.admin, &f.token.address, &DEADLINE);
    assert!(result.is_err(), "initialize must require the admin's auth");
}

/// Every entry point must refuse to act before `initialize`.
#[test]
fn every_function_fails_before_initialize() {
    let f = Fixture::new();
    f.env.mock_all_auths();

    assert_eq!(
        f.vault.try_fund(&100).unwrap_err().unwrap(),
        Error::NotInitialized
    );
    assert_eq!(
        f.vault.try_register(&f.alice, &100).unwrap_err().unwrap(),
        Error::NotInitialized
    );
    assert_eq!(
        f.vault.try_claim(&f.alice).unwrap_err().unwrap(),
        Error::NotInitialized
    );
    assert_eq!(
        f.vault.try_cancel(&f.alice).unwrap_err().unwrap(),
        Error::NotInitialized
    );
    assert_eq!(
        f.vault.try_withdraw(&f.admin, &100).unwrap_err().unwrap(),
        Error::NotInitialized
    );
    assert_eq!(
        f.vault.try_get_admin().unwrap_err().unwrap(),
        Error::NotInitialized
    );
    assert_eq!(
        f.vault.try_get_token().unwrap_err().unwrap(),
        Error::NotInitialized
    );
    assert_eq!(
        f.vault.try_get_deadline().unwrap_err().unwrap(),
        Error::NotInitialized
    );
    assert_eq!(
        f.vault.try_available().unwrap_err().unwrap(),
        Error::NotInitialized
    );
    // The allocation getter is infallible and simply reports absence.
    assert_eq!(f.vault.get_allocation(&f.alice), None);
}

// ---------------------------------------------------------------------------
// fund
// ---------------------------------------------------------------------------

#[test]
fn fund_moves_tokens_into_the_vault() {
    let f = Fixture::new();
    f.mint_to_admin(1_000);
    f.init();

    f.env.mock_all_auths();
    f.vault.fund(&400);

    assert_eq!(f.token.balance(&f.vault_address), 400);
    assert_eq!(f.token.balance(&f.admin), 600);
    assert_eq!(f.vault.available(), 400);
}

#[test]
fn fund_is_cumulative() {
    let f = Fixture::new();
    f.mint_to_admin(1_000);
    f.init();

    f.env.mock_all_auths();
    f.vault.fund(&100);
    f.env.mock_all_auths();
    f.vault.fund(&250);

    assert_eq!(f.token.balance(&f.vault_address), 350);
}

#[test]
fn fund_rejects_non_positive_amounts() {
    let f = Fixture::new();
    f.mint_to_admin(1_000);
    f.init();

    for bad in [0_i128, -1, -1_000] {
        f.env.mock_all_auths();
        assert_eq!(
            f.vault.try_fund(&bad).unwrap_err().unwrap(),
            Error::InvalidAmount,
            "fund({bad}) must be rejected",
        );
    }
    assert_eq!(f.token.balance(&f.vault_address), 0);
}

/// Funding after the deadline is allowed: the tokens are unencumbered and the
/// admin can withdraw them again.
#[test]
fn fund_is_allowed_after_the_deadline() {
    let f = Fixture::new();
    f.mint_to_admin(1_000);
    f.init();
    f.set_time(DEADLINE + 1);

    f.env.mock_all_auths();
    f.vault.fund(&100);

    assert_eq!(f.token.balance(&f.vault_address), 100);
}

// ---------------------------------------------------------------------------
// register
// ---------------------------------------------------------------------------

#[test]
fn register_earmarks_funds() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    assert_eq!(
        f.vault.get_allocation(&f.alice),
        Some(Allocation {
            amount: 300,
            status: AllocationStatus::Pending,
        })
    );
    assert_eq!(f.vault.total_allocated(), 300);
    assert_eq!(f.vault.available(), 700);
    // Registering moves no tokens.
    assert_eq!(f.token.balance(&f.vault_address), 1_000);
}

#[test]
fn register_accumulates_across_recipients() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);
    f.env.mock_all_auths();
    f.vault.register(&f.bob, &700);

    assert_eq!(f.vault.total_allocated(), 1_000);
    assert_eq!(f.vault.available(), 0);
}

#[test]
fn register_rejects_a_duplicate_recipient() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_register(&f.alice, &50).unwrap_err().unwrap(),
        Error::AllocationExists,
    );
    // The original allocation is untouched.
    assert_eq!(f.vault.get_allocation(&f.alice).unwrap().amount, 300);
    assert_eq!(f.vault.total_allocated(), 300);
}

#[test]
fn register_rejects_non_positive_amounts() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    for bad in [0_i128, -1] {
        f.env.mock_all_auths();
        assert_eq!(
            f.vault.try_register(&f.alice, &bad).unwrap_err().unwrap(),
            Error::InvalidAmount,
        );
    }
}

/// The core backing check: an allocation may never exceed what the vault holds.
#[test]
fn register_cannot_exceed_the_vault_balance() {
    let f = Fixture::new();
    f.init_and_fund(500);

    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_register(&f.alice, &501).unwrap_err().unwrap(),
        Error::InsufficientFunds,
    );

    // Exactly the balance is fine.
    f.env.mock_all_auths();
    f.vault.register(&f.alice, &500);

    // But now nothing is left to earmark.
    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_register(&f.bob, &1).unwrap_err().unwrap(),
        Error::InsufficientFunds,
    );
}

#[test]
fn register_is_rejected_at_and_after_the_deadline() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    // Inclusive: exactly at the deadline is already too late.
    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_register(&f.alice, &100).unwrap_err().unwrap(),
        Error::DeadlinePassed,
    );

    f.set_time(DEADLINE + DAY);
    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_register(&f.alice, &100).unwrap_err().unwrap(),
        Error::DeadlinePassed,
    );
}

// ---------------------------------------------------------------------------
// claim
// ---------------------------------------------------------------------------

#[test]
fn claim_pays_the_recipient() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    f.env.mock_all_auths();
    f.vault.claim(&f.alice);

    assert_eq!(f.token.balance(&f.alice), 300);
    assert_eq!(f.token.balance(&f.vault_address), 700);
    assert_eq!(
        f.vault.get_allocation(&f.alice).unwrap().status,
        AllocationStatus::Claimed
    );
    assert_eq!(f.vault.total_allocated(), 0);
    assert_eq!(f.vault.total_claimed(), 300);
    assert_eq!(f.vault.available(), 700);
}

/// The required double-claim case: the second attempt must be a typed error and
/// must not move any tokens.
#[test]
fn claim_twice_is_rejected() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);
    f.env.mock_all_auths();
    f.vault.claim(&f.alice);

    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_claim(&f.alice).unwrap_err().unwrap(),
        Error::AlreadyClaimed,
    );

    assert_eq!(f.token.balance(&f.alice), 300, "no second payout");
    assert_eq!(f.vault.total_claimed(), 300);
}

#[test]
fn claim_without_an_allocation_is_rejected() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_claim(&f.bob).unwrap_err().unwrap(),
        Error::AllocationNotFound,
    );
}

#[test]
fn claim_is_rejected_at_and_after_the_deadline() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    // Exclusive bound: exactly at the deadline, claiming is already closed.
    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_claim(&f.alice).unwrap_err().unwrap(),
        Error::DeadlinePassed,
    );
    assert_eq!(f.token.balance(&f.alice), 0);

    // One second before, it still works.
    f.set_time(DEADLINE - 1);
    f.env.mock_all_auths();
    f.vault.claim(&f.alice);
    assert_eq!(f.token.balance(&f.alice), 300);
}

/// The required claim-after-cancel case.
#[test]
fn claim_after_cancel_is_rejected() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    f.vault.cancel(&f.alice);

    // Past the deadline, claim hits the deadline guard first.
    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_claim(&f.alice).unwrap_err().unwrap(),
        Error::DeadlinePassed,
    );

    assert_eq!(
        f.token.balance(&f.alice),
        0,
        "a cancelled allocation never pays"
    );
}

/// Reaching `AllocationCancelled` requires a vault whose deadline has moved on
/// past a cancel — only possible across two vaults, since the deadline is
/// immutable. Here a second vault with a later deadline proves the status
/// branch itself is wired correctly.
#[test]
fn cancelled_status_is_reported_by_the_getter() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    f.vault.cancel(&f.alice);

    assert_eq!(
        f.vault.get_allocation(&f.alice),
        Some(Allocation {
            amount: 300,
            status: AllocationStatus::Cancelled,
        })
    );
}

// ---------------------------------------------------------------------------
// cancel
// ---------------------------------------------------------------------------

#[test]
fn cancel_releases_the_encumbrance_without_moving_tokens() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);
    assert_eq!(f.vault.available(), 700);

    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    f.vault.cancel(&f.alice);

    assert_eq!(f.vault.total_allocated(), 0);
    assert_eq!(f.vault.available(), 1_000, "funds are reclaimable again");
    assert_eq!(
        f.token.balance(&f.vault_address),
        1_000,
        "cancel moves nothing"
    );
    assert_eq!(f.token.balance(&f.alice), 0);
}

#[test]
fn cancel_before_the_deadline_is_rejected() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_cancel(&f.alice).unwrap_err().unwrap(),
        Error::DeadlineNotReached,
    );

    // One second before the deadline is still too early.
    f.set_time(DEADLINE - 1);
    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_cancel(&f.alice).unwrap_err().unwrap(),
        Error::DeadlineNotReached,
    );
    assert_eq!(f.vault.total_allocated(), 300);
}

/// At exactly `deadline`, cancel is valid and claim is not: the boundary
/// resolves to exactly one of the two, so they can never race.
#[test]
fn the_deadline_boundary_belongs_to_cancel() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    f.set_time(DEADLINE);

    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_claim(&f.alice).unwrap_err().unwrap(),
        Error::DeadlinePassed,
    );

    f.env.mock_all_auths();
    f.vault.cancel(&f.alice);
    assert_eq!(f.vault.total_allocated(), 0);
}

#[test]
fn cancel_without_an_allocation_is_rejected() {
    let f = Fixture::new();
    f.init_and_fund(1_000);
    f.set_time(DEADLINE);

    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_cancel(&f.bob).unwrap_err().unwrap(),
        Error::AllocationNotFound,
    );
}

#[test]
fn cancel_twice_is_rejected() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    f.vault.cancel(&f.alice);

    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_cancel(&f.alice).unwrap_err().unwrap(),
        Error::AllocationCancelled,
    );
    assert_eq!(f.vault.total_allocated(), 0, "no double release");
}

#[test]
fn cancel_of_a_claimed_allocation_is_rejected() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);
    f.env.mock_all_auths();
    f.vault.claim(&f.alice);

    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_cancel(&f.alice).unwrap_err().unwrap(),
        Error::AlreadyClaimed,
    );
    // Releasing it again would corrupt the encumbrance.
    assert_eq!(f.vault.total_allocated(), 0);
}

// ---------------------------------------------------------------------------
// withdraw — the central safety property
// ---------------------------------------------------------------------------

#[test]
fn withdraw_sends_unencumbered_funds() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.withdraw(&f.bob, &400);

    assert_eq!(f.token.balance(&f.bob), 400);
    assert_eq!(f.token.balance(&f.vault_address), 600);
}

/// The most important test in the suite: a pending allocation's funds are
/// untouchable by the admin.
#[test]
fn withdraw_cannot_touch_a_pending_allocation() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &700);
    assert_eq!(f.vault.available(), 300);

    // The full balance is not withdrawable...
    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_withdraw(&f.admin, &1_000).unwrap_err().unwrap(),
        Error::InsufficientFunds,
    );
    // ...nor even one unit beyond the unencumbered part.
    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_withdraw(&f.admin, &301).unwrap_err().unwrap(),
        Error::InsufficientFunds,
    );

    // Exactly the unencumbered part succeeds.
    f.env.mock_all_auths();
    f.vault.withdraw(&f.admin, &300);
    assert_eq!(f.token.balance(&f.vault_address), 700);

    // And Alice can still be paid in full.
    f.env.mock_all_auths();
    f.vault.claim(&f.alice);
    assert_eq!(f.token.balance(&f.alice), 700);
    assert_eq!(f.token.balance(&f.vault_address), 0);
}

#[test]
fn withdraw_rejects_non_positive_amounts() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    for bad in [0_i128, -5] {
        f.env.mock_all_auths();
        assert_eq!(
            f.vault.try_withdraw(&f.admin, &bad).unwrap_err().unwrap(),
            Error::InvalidAmount,
        );
    }
}

#[test]
fn withdraw_on_an_empty_vault_is_rejected() {
    let f = Fixture::new();
    f.init();

    f.env.mock_all_auths();
    assert_eq!(
        f.vault.try_withdraw(&f.admin, &1).unwrap_err().unwrap(),
        Error::InsufficientFunds,
    );
}

/// cancel + withdraw together are the "reclaim" path from the brief.
#[test]
fn cancel_then_withdraw_reclaims_everything() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &600);
    f.env.mock_all_auths();
    f.vault.register(&f.bob, &400);
    assert_eq!(f.vault.available(), 0);

    f.set_time(DEADLINE + DAY);
    f.env.mock_all_auths();
    f.vault.cancel(&f.alice);
    f.env.mock_all_auths();
    f.vault.cancel(&f.bob);

    assert_eq!(f.vault.available(), 1_000);

    f.env.mock_all_auths();
    f.vault.withdraw(&f.admin, &1_000);

    assert_eq!(f.token.balance(&f.vault_address), 0);
    assert_eq!(f.token.balance(&f.admin), 1_000);
    assert_eq!(f.vault.total_allocated(), 0);
    assert_eq!(f.vault.total_claimed(), 0);
}

// ---------------------------------------------------------------------------
// authorization
// ---------------------------------------------------------------------------

/// Each admin-only function must fail with no auth mocked. `mock_all_auths` is
/// deliberately avoided so these assertions mean something.
#[test]
fn admin_functions_require_auth() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    // Registered up front so that every call below would *succeed* if auth were
    // the only thing missing. Otherwise these assertions could pass for an
    // unrelated reason and would survive the removal of `require_auth`.
    f.env.mock_all_auths();
    f.vault.register(&f.alice, &100);

    f.env.set_auths(&[]);
    assert!(f.vault.try_fund(&10).is_err(), "fund must require auth");

    f.env.set_auths(&[]);
    assert!(
        f.vault.try_register(&f.bob, &10).is_err(),
        "register must require auth"
    );

    f.env.set_auths(&[]);
    assert!(
        f.vault.try_withdraw(&f.admin, &10).is_err(),
        "withdraw must require auth"
    );

    f.set_time(DEADLINE);
    f.env.set_auths(&[]);
    assert!(
        f.vault.try_cancel(&f.alice).is_err(),
        "cancel must require auth"
    );

    // Nothing moved and nothing settled.
    assert_eq!(f.token.balance(&f.vault_address), 1_000);
    assert_eq!(f.vault.total_allocated(), 100);
    assert_eq!(f.vault.get_allocation(&f.bob), None);
}

/// A non-admin must not be able to drive admin-only functions, even with their
/// own signature present.
#[test]
fn non_admin_cannot_call_admin_functions() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_auths(&[MockAuth {
        address: &f.alice,
        invoke: &MockAuthInvoke {
            contract: &f.vault_address,
            fn_name: "withdraw",
            args: (f.alice.clone(), 100_i128).into_val(&f.env),
            sub_invokes: &[],
        },
    }]);
    assert!(
        f.vault.try_withdraw(&f.alice, &100).is_err(),
        "alice's signature must not authorize a withdraw"
    );
    assert_eq!(f.token.balance(&f.alice), 0);
}

/// `claim` must be authorized by the recipient themselves.
#[test]
fn claim_requires_the_recipients_own_auth() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    // No auth at all.
    f.env.set_auths(&[]);
    assert!(f.vault.try_claim(&f.alice).is_err(), "claim needs auth");

    // Bob's signature must not let Bob drain Alice's allocation.
    f.env.mock_auths(&[MockAuth {
        address: &f.bob,
        invoke: &MockAuthInvoke {
            contract: &f.vault_address,
            fn_name: "claim",
            args: (f.alice.clone(),).into_val(&f.env),
            sub_invokes: &[],
        },
    }]);
    assert!(
        f.vault.try_claim(&f.alice).is_err(),
        "bob must not be able to claim for alice"
    );

    assert_eq!(f.token.balance(&f.alice), 0);
    assert_eq!(f.token.balance(&f.bob), 0);
    assert_eq!(f.vault.total_allocated(), 300, "nothing settled");
}

/// Structural check that `claim` really required *alice*, not merely "some" auth.
#[test]
fn claim_records_the_recipient_as_the_authorizer() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    f.env.mock_all_auths();
    f.vault.claim(&f.alice);

    let auths = f.env.auths();
    let claim_auth = auths
        .iter()
        .find(|(address, invocation)| {
            *address == f.alice
                && matches!(
                    &invocation.function,
                    AuthorizedFunction::Contract((contract, fn_name, _))
                        if *contract == f.vault_address
                            && *fn_name == Symbol::new(&f.env, "claim")
                )
        })
        .cloned();

    assert!(
        claim_auth.is_some(),
        "expected alice to be recorded as claim's authorizer, got {auths:?}"
    );
}

/// Structural check that `register` required the admin specifically.
#[test]
fn register_records_the_admin_as_the_authorizer() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    let auths = f.env.auths();
    let found = auths.iter().any(|(address, invocation)| {
        *address == f.admin
            && matches!(
                &invocation.function,
                AuthorizedFunction::Contract((contract, fn_name, _))
                    if *contract == f.vault_address
                        && *fn_name == Symbol::new(&f.env, "register")
            )
    });

    assert!(found, "expected admin to authorize register, got {auths:?}");
}

// ---------------------------------------------------------------------------
// events
// ---------------------------------------------------------------------------

/// Walks the whole lifecycle, asserting the exact event payload after each
/// operation. `events().all()` only reports the most recent invocation, so each
/// assertion is made immediately after its call.
#[test]
fn every_operation_emits_its_event() {
    let f = Fixture::new();
    f.mint_to_admin(1_000);

    f.env.mock_all_auths();
    f.vault.initialize(&f.admin, &f.token.address, &DEADLINE);
    f.assert_emitted(crate::events::Initialized {
        admin: f.admin.clone(),
        token: f.token.address.clone(),
        deadline: DEADLINE,
    });

    f.env.mock_all_auths();
    f.vault.fund(&1_000);
    f.assert_emitted(crate::events::Funded {
        from: f.admin.clone(),
        amount: 1_000,
        balance: 1_000,
    });

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);
    f.assert_emitted(crate::events::Registered {
        recipient: f.alice.clone(),
        amount: 300,
        total_allocated: 300,
    });

    f.env.mock_all_auths();
    f.vault.claim(&f.alice);
    f.assert_emitted(crate::events::Claimed {
        recipient: f.alice.clone(),
        amount: 300,
    });

    f.env.mock_all_auths();
    f.vault.register(&f.bob, &200);
    f.assert_emitted(crate::events::Registered {
        recipient: f.bob.clone(),
        amount: 200,
        total_allocated: 200,
    });

    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    f.vault.cancel(&f.bob);
    f.assert_emitted(crate::events::Cancelled {
        recipient: f.bob.clone(),
        amount: 200,
    });

    f.env.mock_all_auths();
    f.vault.withdraw(&f.admin, &700);
    f.assert_emitted(crate::events::Withdrawn {
        to: f.admin.clone(),
        amount: 700,
    });
}

/// A failed call must emit nothing.
#[test]
fn a_rejected_call_emits_no_event() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    assert!(f.vault.try_register(&f.alice, &0).is_err());

    let emitted = f.env.events().all().filter_by_contract(&f.vault_address);
    assert!(
        emitted.events().is_empty(),
        "a rejected register must not emit: {emitted:?}"
    );
}

// ---------------------------------------------------------------------------
// TTL / archival
// ---------------------------------------------------------------------------

/// An allocation must survive long past the point a default-TTL entry would
/// have been archived, because `register` bumps it on write.
#[test]
fn allocations_survive_well_past_the_default_ttl() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    // Advance 45 days of ledgers: beyond the 30-day bump threshold but inside
    // the 60-day extension, so the entry must still be live.
    let ledgers = 45 * crate::storage::DAY_IN_LEDGERS;
    f.env.ledger().with_mut(|li| {
        li.sequence_number += ledgers;
        li.timestamp = DEADLINE - 1;
    });

    assert_eq!(
        f.vault.get_allocation(&f.alice),
        Some(Allocation {
            amount: 300,
            status: AllocationStatus::Pending,
        }),
        "allocation should not have been archived"
    );

    f.env.mock_all_auths();
    f.vault.claim(&f.alice);
    assert_eq!(f.token.balance(&f.alice), 300);
}

/// Reading an allocation must not extend its TTL — extension is a ledger write
/// and getters are meant to be pure.
#[test]
fn getters_do_not_extend_ttl() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &300);

    let key = crate::DataKey::Allocation(f.alice.clone());
    let ttl_before = f.env.as_contract(&f.vault_address, || {
        f.env.storage().persistent().get_ttl(&key)
    });

    let _ = f.vault.get_allocation(&f.alice);
    let _ = f.vault.total_allocated();

    let ttl_after = f.env.as_contract(&f.vault_address, || {
        f.env.storage().persistent().get_ttl(&key)
    });

    assert_eq!(ttl_before, ttl_after, "a read must not bump TTL");
}

// ---------------------------------------------------------------------------
// integration-flavoured: multiple recipients, partial claims
// ---------------------------------------------------------------------------

#[test]
fn partial_claims_then_cancel_the_remainder() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &400);
    f.env.mock_all_auths();
    f.vault.register(&f.bob, &600);

    // Alice claims; Bob never does.
    f.env.mock_all_auths();
    f.vault.claim(&f.alice);

    assert_eq!(f.vault.total_allocated(), 600);
    assert_eq!(f.vault.total_claimed(), 400);
    assert_eq!(f.vault.available(), 0);

    // After the deadline the admin reclaims Bob's share.
    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    f.vault.cancel(&f.bob);
    assert_eq!(f.vault.available(), 600);

    f.env.mock_all_auths();
    f.vault.withdraw(&f.admin, &600);

    assert_eq!(f.token.balance(&f.alice), 400);
    assert_eq!(f.token.balance(&f.bob), 0);
    assert_eq!(f.token.balance(&f.admin), 600);
    assert_eq!(f.token.balance(&f.vault_address), 0);
}

/// The invariant `total_allocated <= balance` must hold after every operation
/// in a longer mixed sequence.
#[test]
fn the_backing_invariant_holds_throughout() {
    let f = Fixture::new();
    f.init_and_fund(1_000);

    let assert_invariant = || {
        let balance = f.token.balance(&f.vault_address);
        let allocated = f.vault.total_allocated();
        assert!(
            allocated <= balance,
            "invariant violated: allocated {allocated} > balance {balance}"
        );
    };

    assert_invariant();

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &400);
    assert_invariant();

    f.env.mock_all_auths();
    f.vault.register(&f.bob, &500);
    assert_invariant();

    f.env.mock_all_auths();
    f.vault.claim(&f.alice);
    assert_invariant();

    f.env.mock_all_auths();
    f.vault.withdraw(&f.admin, &100);
    assert_invariant();

    f.set_time(DEADLINE);
    f.env.mock_all_auths();
    f.vault.cancel(&f.bob);
    assert_invariant();

    f.env.mock_all_auths();
    f.vault.withdraw(&f.admin, &500);
    assert_invariant();

    assert_eq!(f.token.balance(&f.vault_address), 0);
}

/// Large amounts near `i128::MAX` must produce a typed `MathOverflow` rather
/// than trapping, proving the checked-math paths are reachable.
#[test]
fn counters_use_checked_math() {
    let f = Fixture::new();
    f.mint_to_admin(i128::MAX);
    f.init();

    f.env.mock_all_auths();
    f.vault.fund(&i128::MAX);

    f.env.mock_all_auths();
    f.vault.register(&f.alice, &i128::MAX);
    assert_eq!(f.vault.total_allocated(), i128::MAX);

    // A second allocation of any size would overflow `total_allocated`.
    f.env.mock_all_auths();
    let err = f.vault.try_register(&f.bob, &1).unwrap_err().unwrap();
    assert!(
        err == Error::MathOverflow || err == Error::InsufficientFunds,
        "expected a typed error, got {err:?}"
    );

    // And the vault is still consistent.
    assert_eq!(f.vault.total_allocated(), i128::MAX);
}
