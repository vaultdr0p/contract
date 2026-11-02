use soroban_sdk::contracterror;

/// Every expected failure mode of the vault.
///
/// Codes are part of the contract's public interface and are stable: existing
/// variants must never be renumbered, only appended to.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// `initialize` was called on an already-initialized vault.
    AlreadyInitialized = 1,
    /// A function was called before `initialize`.
    NotInitialized = 2,
    /// An amount was zero or negative.
    InvalidAmount = 3,
    /// `initialize` was given a deadline at or before the current ledger time.
    InvalidDeadline = 4,
    /// `register` was called for an address that already has an allocation.
    AllocationExists = 5,
    /// `claim` or `cancel` was called for an address with no allocation.
    AllocationNotFound = 6,
    /// `claim` was called on an allocation that was already claimed.
    AlreadyClaimed = 7,
    /// `claim` was called on an allocation that the admin cancelled.
    AllocationCancelled = 8,
    /// `register` would exceed the vault balance, or `withdraw` would dip into
    /// funds already encumbered by a pending allocation.
    InsufficientFunds = 9,
    /// `cancel` was called before the deadline.
    DeadlineNotReached = 10,
    /// `register` or `claim` was called at or after the deadline.
    DeadlinePassed = 11,
    /// Checked arithmetic over- or underflowed.
    MathOverflow = 12,
}
