use soroban_sdk::contracterror;

/// Every expected failure mode of the vault.
///
/// Codes are part of the contract's public interface and are stable: existing
/// variants must never be renumbered, only appended to.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Not returned since v0.1.0, which replaced `initialize` with a
    /// constructor that the host runs only once. Kept so codes stay stable;
    /// a legacy (v0.0.0) vault can still return it.
    AlreadyInitialized = 1,
    /// The vault has no configuration. Since v0.1.0 the constructor always
    /// writes it, so a deployed vault does not return this; a legacy (v0.0.0)
    /// vault returned it before `initialize`.
    NotInitialized = 2,
    /// An amount was zero or negative.
    InvalidAmount = 3,
    /// The constructor was given a deadline at or before the current ledger
    /// time. The deployer sees a generic host error, not this code; see
    /// `__constructor`.
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
