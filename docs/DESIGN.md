# Disbursement Vault — Design

Soroban contract that lets an **admin** fund a vault with a single token
(Stellar Asset Contract / USDC), register **recipients** with fixed
allocations, and lets each recipient **claim** their allocation. After a
**deadline**, the admin can cancel unclaimed allocations and reclaim the
unencumbered balance.

- **Contract crate:** `contracts/disbursement-vault`
- **SDK:** `soroban-sdk 29.0.1` (latest stable; `30.0.0-rc.1` skipped as pre-release)
- **Amount type:** `i128` throughout, matching the SOROBAN token interface

---

## 1. Roles and lifecycle

| Role | Capability |
|---|---|
| `admin` | `fund`, `register`, `cancel`, `withdraw` |
| `recipient` | `claim` (own allocation only) |
| anyone | read-only getters |

```
initialize(admin, token, deadline)
        │
        ▼
     fund(amount)  ◄──────────┐  admin tops up vault balance
        │                     │
        ▼                     │
  register(recipient, amount) ─┘  admin earmarks funds (only while now < deadline)
        │
        ├──  now <  deadline ──►  claim(recipient)      → recipient receives tokens
        │
        └──  now >= deadline ──►  cancel(recipient)     → frees the allocation
                                  withdraw(to, amount)  → admin reclaims unencumbered funds
```

### The core invariant

> Every registered-but-unclaimed allocation is fully backed by tokens
> actually held by the contract.

Formally, with `balance = token.balance(contract_address)`:

```
total_allocated  <=  balance
```

where `total_allocated` is the sum of all `Pending` allocations. Every
state-changing function preserves this:

| Function | `total_allocated` | `balance` |
|---|---|---|
| `fund(amount)` | — | `+amount` |
| `register(_, amount)` | `+amount` (rejected unless it still fits in `balance`) | — |
| `claim` | `-amount` | `-amount` |
| `cancel` | `-amount` | — |
| `withdraw(_, amount)` | — | `-amount` (capped at `balance - total_allocated`) |

Because `withdraw` is capped at the **unencumbered** balance
(`balance - total_allocated`), the admin can never withdraw tokens that
are owed to a recipient with a `Pending` allocation. This is the single
most important safety property of the contract.

Note that `balance` is read live from the token contract rather than
mirrored in our own storage. A third party transferring tokens directly to
the contract address simply increases the unencumbered balance; it cannot
break the invariant, and the admin can allocate or withdraw those funds.

---

## 2. Storage layout

### Instance storage — small, bounded, shared by every invocation

Instance storage is one ledger entry that is loaded on *every* call and
shares the contract instance's TTL. It must stay small and of bounded
size, so it holds only singleton config and counters — never per-recipient
data.

| Key | Type | Purpose |
|---|---|---|
| `DataKey::Config` | `Config` | admin, token, deadline. Its **presence is the initialized flag** |
| `DataKey::TotalAllocated` | `i128` | sum of `Pending` allocations (the encumbrance) |
| `DataKey::TotalClaimed` | `i128` | lifetime claimed, for reporting only |

```rust
#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admin: Address,
    pub token: Address,
    pub deadline: u64, // unix seconds
}
```

### Persistent storage — unbounded, per-recipient, independently archivable

| Key | Type | Purpose |
|---|---|---|
| `DataKey::Allocation(Address)` | `Allocation` | one entry per recipient |

```rust
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AllocationStatus { Pending, Claimed, Cancelled }

#[contracttype]
#[derive(Clone)]
pub struct Allocation {
    pub amount: i128,
    pub status: AllocationStatus,
}
```

**Why persistent, not instance:** the recipient set is unbounded. A
`Map<Address, Allocation>` in instance storage would (a) be read and
written in full on every single call, (b) hit the ledger-entry size limit
at a few hundred recipients, and (c) force all recipients to share one
TTL. One persistent entry per recipient gives each recipient an
independent footprint and TTL, and keeps per-call cost O(1).

**Why `temporary` is wrong here:** temporary entries are permanently
deleted at expiry with no possibility of restoration. An allocation
represents a claim on real funds and must be restorable, so it is
`persistent`.

**Why keep a `status` rather than deleting on claim:** retaining the entry
makes a double-claim distinguishable from a never-registered address
(`AlreadyClaimed` vs `AllocationNotFound`) and makes a claim-after-cancel
report `AllocationCancelled`. That precision is worth the storage rent,
and it leaves an on-chain audit trail of the disbursement.

---

## 3. Function list

All amounts are `i128`. All state-changing functions call `require_auth()`
and return `Result<_, Error>`.

### State-changing

| Function | Auth | Guards | Effect |
|---|---|---|---|
| `initialize(admin, token, deadline)` | `admin` | not already initialized; `deadline > now` | writes `Config`, zeroes counters |
| `fund(amount)` | `admin` | initialized; `amount > 0` | `token.transfer(admin → contract, amount)` |
| `register(recipient, amount)` | `admin` | initialized; `amount > 0`; `now < deadline`; no existing allocation; `total_allocated + amount <= balance` | writes `Pending` allocation; `total_allocated += amount` |
| `claim(recipient)` | `recipient` | initialized; allocation exists and is `Pending`; `now < deadline` | `token.transfer(contract → recipient, amount)`; status `Claimed`; `total_allocated -= amount`; `total_claimed += amount` |
| `cancel(recipient)` | `admin` | initialized; `now >= deadline`; allocation exists and is `Pending` | status `Cancelled`; `total_allocated -= amount` |
| `withdraw(to, amount)` | `admin` | initialized; `amount > 0`; `amount <= balance - total_allocated` | `token.transfer(contract → to, amount)` |

### Read-only (no auth)

`get_admin()`, `get_token()`, `get_deadline()`, `get_allocation(recipient)
-> Option<Allocation>`, `total_allocated()`, `total_claimed()`,
`available()` (= unencumbered balance).

### Deliberate boundary decisions

- **`claim` requires `now < deadline`.** The deadline is a hard cutover:
  before it, funds belong to recipients; at or after it, they are the
  admin's to reclaim. A single boundary means `claim` and `cancel` can
  never both be valid in the same ledger, so there is no race between a
  recipient claiming and the admin cancelling. Boundary is exclusive for
  `claim` and inclusive for `cancel`, so exactly one applies at
  `now == deadline`.
- **`register` requires `now < deadline`.** Registering an allocation that
  could never be claimed would be a silent footgun.
- **`register` rejects an existing allocation** (any status) rather than
  overwriting, so an allocation can never be silently changed or reset
  under a recipient. Re-running a disbursement round for the same address
  is out of scope; it would need a round id in the key.
- **`cancel` does not transfer.** It only frees the encumbrance;
  `withdraw` moves tokens. Splitting them keeps each function's authority
  narrow, lets the admin cancel many allocations and withdraw once, and
  means `cancel` cannot fail on a token error.

---

## 4. Events

Emitted with SDK 29's typed `#[contractevent]` macro, which generates the
topic/data encoding and gives the tests a strongly-typed assertion target.
The `recipient` field is a topic so indexers can filter per recipient.

| Event | Topics | Data |
|---|---|---|
| `initialized` | `admin` | `token`, `deadline` |
| `funded` | `from` | `amount`, `balance` |
| `registered` | `recipient` | `amount`, `total_allocated` |
| `claimed` | `recipient` | `amount` |
| `cancelled` | `recipient` | `amount` |
| `withdrawn` | `to` | `amount` |

```rust
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claimed {
    #[topic]
    pub recipient: Address,
    pub amount: i128,
}
```

The four required by the brief are `funded`, `registered`, `claimed`, and
`cancelled`; `initialized` and `withdrawn` are included so that the full
lifecycle is reconstructible from the event stream alone.

---

## 5. Errors

A typed `#[contracterror] #[repr(u32)]` enum. Every expected failure
returns `Err`; the contract does not `panic!` on any reachable,
caller-triggered path. Codes are stable and must only ever be appended to.

```rust
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized  = 1,
    NotInitialized      = 2,
    InvalidAmount       = 3,
    InvalidDeadline     = 4,
    AllocationExists    = 5,
    AllocationNotFound  = 6,
    AlreadyClaimed      = 7,
    AllocationCancelled = 8,
    InsufficientFunds   = 9,
    DeadlineNotReached  = 10,
    DeadlinePassed      = 11,
    MathOverflow        = 12,
}
```

| Code | Raised by |
|---|---|
| `AlreadyInitialized` | second `initialize` |
| `NotInitialized` | any function before `initialize` |
| `InvalidAmount` | `amount <= 0` in `fund` / `register` / `withdraw` |
| `InvalidDeadline` | `initialize` with `deadline <= now` |
| `AllocationExists` | `register` for an address that already has one |
| `AllocationNotFound` | `claim` / `cancel` for an unregistered address |
| `AlreadyClaimed` | double `claim` |
| `AllocationCancelled` | `claim` after `cancel` |
| `InsufficientFunds` | `register` beyond vault balance, or `withdraw` beyond unencumbered balance |
| `DeadlineNotReached` | `cancel` before the deadline |
| `DeadlinePassed` | `register` or `claim` at/after the deadline |
| `MathOverflow` | any `checked_add` / `checked_sub` returning `None` |

**Checked math.** Every arithmetic op on an amount or counter uses
`checked_add` / `checked_sub` and maps `None` to `MathOverflow`. The
release profile already sets `overflow-checks = true`, but that panics —
and a panic in a Soroban contract is an unrecoverable trap, not a typed
error. Explicit checked math keeps these cases inside the `Error` enum.

---

## 6. TTL and archival strategy

Soroban charges rent for ledger entries and **archives** them when their
TTL lapses. Instance entries archive with the contract; persistent entries
archive individually and must be restored before use.

```rust
const DAY_IN_LEDGERS: u32 = 17_280;              // ~5s per ledger

const INSTANCE_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
const INSTANCE_EXTEND:    u32 = 60 * DAY_IN_LEDGERS;

const ALLOCATION_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
const ALLOCATION_EXTEND:    u32 = 60 * DAY_IN_LEDGERS;
```

Both bumps sit well inside the network `max_entry_ttl` (~180 days), so
`extend_ttl` can never fail on a limit.

**Policy — "touch it, bump it":**

1. **Instance.** Every state-changing function calls
   `env.storage().instance().extend_ttl(INSTANCE_THRESHOLD, INSTANCE_EXTEND)`.
   This also extends the contract's Wasm code entry, so an actively used
   vault never archives. The bump happens *after* the guards pass, so a
   failing call cannot be used to bump TTL for free.
2. **Allocations.** `register` bumps the new entry's TTL on write, and
   `claim` / `cancel` bump on read. `extend_ttl` with a threshold is a
   no-op when the entry already has more than `THRESHOLD` ledgers left, so
   this is cheap for frequent callers.
3. **Read-only getters do not bump.** TTL extension is a ledger write;
   keeping getters pure means they stay cheap and simulatable, and a
   third-party read cannot subsidise someone else's rent.

**Archival of an unclaimed allocation.** If a recipient never claims and
their entry's TTL lapses, the entry is archived rather than lost. Claiming
then requires a `RestoreFootprint` operation first; the stellar CLI and JS
SDK do this automatically on simulation. Two things keep this rare in
practice:

- a 60-day bump at registration covers any reasonable deadline, and
- the recommended operational pattern is to set `deadline` well within one
  bump window of `initialize`.

Because the entry is `persistent`, even a long-archived allocation remains
provably claimable after restoration — the funds are never stranded. The
`total_allocated` counter lives in instance storage and so cannot drift
out of sync with an archived allocation: archival does not change the
counter, and restoration brings back the exact prior value.

---

## 7. Test plan

Using `soroban-sdk` `testutils` with `env.register_stellar_asset_contract_v2`
for a real SAC token, `env.mock_auths` for precise per-address auth
assertions, and `env.ledger().set_timestamp` to cross the deadline.

**Happy path:** initialize → fund → register two recipients → both claim →
balances, counters, and events all correct.

**Explicitly required by the brief:**

- **Auth failures** — `register` / `fund` / `cancel` / `withdraw` invoked
  without the admin's signature, and `claim(a)` invoked with `b`'s
  signature. Asserted with scoped `mock_auths`, not `mock_all_auths`.
- **Double-claim** — second `claim` returns `AlreadyClaimed` and transfers
  nothing.
- **Claim after cancel** — `cancel` past the deadline, then `claim`
  returns `AllocationCancelled`.
- **Expiry** — `claim` at/after the deadline returns `DeadlinePassed`;
  `cancel` before it returns `DeadlineNotReached`; the `now == deadline`
  boundary resolves to exactly one of the two.

**Every other branch:** double `initialize`; every function before
`initialize`; non-positive amounts; `register` over balance; duplicate
`register`; `claim` / `cancel` for an unknown address; `withdraw` beyond
the unencumbered balance; and the key safety test — **`withdraw` cannot
touch a `Pending` allocation's funds**, verified by funding, registering,
then asserting `withdraw` of the full balance fails while `withdraw` of
exactly the unencumbered part succeeds.

Auth coverage is also asserted structurally via `env.auths()`, confirming
each state-changing function actually required the address it claims to.

---

## 8. Out of scope

Deliberately excluded to keep the contract auditable:

- admin transfer / two-step ownership handover
- multiple concurrent disbursement rounds (would need a round id in the key)
- batch `register` / `cancel` (callers can batch at the transaction level)
- per-allocation deadlines or vesting schedules
- upgradeability (`update_current_contract_wasm`)
- multi-token vaults — one vault, one token
