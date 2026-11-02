use soroban_sdk::{contracttype, Address};

/// Vault-wide configuration. Written once by `initialize`; its presence in
/// instance storage is what marks the vault as initialized.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// The only address permitted to fund, register, cancel and withdraw.
    pub admin: Address,
    /// The single token this vault disburses (a SAC such as USDC).
    pub token: Address,
    /// Unix seconds. Before it, recipients may claim; at or after it, the
    /// admin may cancel and reclaim.
    pub deadline: u64,
}

/// Lifecycle state of one recipient's allocation.
///
/// Allocations are retained after claim or cancel rather than deleted, so that
/// a double claim is distinguishable from a never-registered address.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationStatus {
    /// Registered and owed to the recipient.
    Pending,
    /// Paid out to the recipient.
    Claimed,
    /// Cancelled by the admin after the deadline.
    Cancelled,
}

/// A single recipient's entitlement.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allocation {
    pub amount: i128,
    pub status: AllocationStatus,
}

/// Storage keys.
///
/// `Config`, `TotalAllocated` and `TotalClaimed` live in instance storage;
/// `Allocation` lives in persistent storage, one entry per recipient.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    /// Sum of all `Pending` allocations: the vault's encumbrance.
    TotalAllocated,
    /// Lifetime total claimed, for reporting only.
    TotalClaimed,
    Allocation(Address),
}
