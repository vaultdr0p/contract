//! Storage accessors and TTL policy.
//!
//! Singletons (config, counters) live in instance storage; per-recipient
//! allocations live in persistent storage so each has an independent footprint
//! and TTL, and so per-call cost stays O(1) regardless of recipient count.
//!
//! TTL policy is "touch it, bump it": every state-changing function extends the
//! instance, and any function that reads or writes an allocation extends that
//! allocation. Bumps are always applied *after* guards pass, so a failing call
//! cannot be used to extend TTL for free. Read-only getters never bump, since
//! extension is a ledger write.

use soroban_sdk::{Address, Env};

use crate::error::Error;
use crate::types::{Allocation, Config, DataKey};

/// Ledgers per day at the ~5s close time Soroban targets.
pub(crate) const DAY_IN_LEDGERS: u32 = 17_280;

/// Bump the instance when it has less than 30 days left, out to 60 days.
pub(crate) const INSTANCE_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const INSTANCE_EXTEND: u32 = 60 * DAY_IN_LEDGERS;

/// Same policy for allocations. Both sit well inside the network's
/// `max_entry_ttl` (~180 days), so `extend_ttl` can never fail on a limit.
pub(crate) const ALLOCATION_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const ALLOCATION_EXTEND: u32 = 60 * DAY_IN_LEDGERS;

// --- config ---------------------------------------------------------------

pub(crate) fn has_config(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Config)
}

/// Loads the config, mapping absence to `NotInitialized`. Every state-changing
/// function except `initialize` starts here.
pub(crate) fn config(env: &Env) -> Result<Config, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(Error::NotInitialized)
}

pub(crate) fn set_config(env: &Env, config: &Config) {
    env.storage().instance().set(&DataKey::Config, config);
}

// --- counters -------------------------------------------------------------

pub(crate) fn total_allocated(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalAllocated)
        .unwrap_or(0)
}

pub(crate) fn set_total_allocated(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::TotalAllocated, &amount);
}

pub(crate) fn total_claimed(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalClaimed)
        .unwrap_or(0)
}

pub(crate) fn set_total_claimed(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::TotalClaimed, &amount);
}

// --- allocations ----------------------------------------------------------

pub(crate) fn allocation(env: &Env, recipient: &Address) -> Option<Allocation> {
    env.storage()
        .persistent()
        .get(&DataKey::Allocation(recipient.clone()))
}

pub(crate) fn set_allocation(env: &Env, recipient: &Address, allocation: &Allocation) {
    env.storage()
        .persistent()
        .set(&DataKey::Allocation(recipient.clone()), allocation);
}

// --- TTL ------------------------------------------------------------------

/// Extends the contract instance and its Wasm code entry, so an actively used
/// vault never archives.
pub(crate) fn extend_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_THRESHOLD, INSTANCE_EXTEND);
}

/// Extends one allocation entry. A no-op when the entry already has more than
/// `ALLOCATION_THRESHOLD` ledgers left, so this is cheap for frequent callers.
pub(crate) fn extend_allocation(env: &Env, recipient: &Address) {
    env.storage().persistent().extend_ttl(
        &DataKey::Allocation(recipient.clone()),
        ALLOCATION_THRESHOLD,
        ALLOCATION_EXTEND,
    );
}
