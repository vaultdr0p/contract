//! Typed contract events.
//!
//! Each struct carries its own topic/data encoding via `#[contractevent]`, and
//! exposes `.publish(env)`. The address a downstream indexer would filter on is
//! marked `#[topic]`; everything else lands in the event's data map.

use soroban_sdk::{contractevent, Address};

/// Emitted once, by `initialize`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Initialized {
    #[topic]
    pub admin: Address,
    pub token: Address,
    pub deadline: u64,
}

/// Emitted by `fund`. `balance` is the vault's token balance after the transfer.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Funded {
    #[topic]
    pub from: Address,
    pub amount: i128,
    pub balance: i128,
}

/// Emitted by `register`. `total_allocated` is the encumbrance after the write.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Registered {
    #[topic]
    pub recipient: Address,
    pub amount: i128,
    pub total_allocated: i128,
}

/// Emitted by `claim`, after the tokens have moved.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claimed {
    #[topic]
    pub recipient: Address,
    pub amount: i128,
}

/// Emitted by `cancel`. No tokens move; only the encumbrance is released.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cancelled {
    #[topic]
    pub recipient: Address,
    pub amount: i128,
}

/// Emitted by `withdraw`, after the tokens have moved.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Withdrawn {
    #[topic]
    pub to: Address,
    pub amount: i128,
}
