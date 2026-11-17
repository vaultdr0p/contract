# Threat model

What the disbursement vault defends against, how, and what it does not defend
against. The contract is **testnet only and not audited**; see
[SECURITY.md](../SECURITY.md) for reporting and scope.

**Rule for this document:** every claim that the contract prevents something
names the function, the guard in the code, and the test in
[`src/test.rs`](../contracts/disbursement-vault/src/test.rs) that covers it. A
claim with no test says **no test**.

## Assets

| Asset | Why it matters |
|---|---|
| Tokens backing pending allocations | Owed to recipients. Losing them breaks the vault's purpose |
| Unencumbered tokens | Belong to the admin. Must stay withdrawable |
| Allocation records | Define who is owed what, and whether it has been paid |
| Configuration: admin, token, deadline | Decide who controls the vault and when claims close |
| Ledger entry TTLs | An archived entry is unusable until restored |

## Actors and trust

| Actor | Trust |
|---|---|
| **Admin** | Trusted to fund, register and cancel honestly. The contract still stops it from taking tokens that back a pending allocation |
| **Recipient** | Trusted only with its own allocation |
| **Anyone else** | Untrusted. Can call any function, but needs the right signature to change state |
| **Token contract** | Trusted to behave as a correct SEP-41 token, such as a Stellar Asset Contract. Not checked by the contract |
| **Network** | Trusted to enforce authorization correctly and to keep ledger time non-decreasing |

## The invariant

The sum of all `Pending` allocations (`total_allocated`) never exceeds the
vault's token balance. Two guards keep it:

- `register` refuses an allocation the balance cannot cover.
- `withdraw` is capped at `balance - total_allocated`.

`the_backing_invariant_holds_throughout` checks it after each step of a mixed
sequence of calls.

## What the contract prevents

### Control of the vault

| Threat | Function and guard | Test |
|---|---|---|
| Configuring a freshly deployed vault before its deployer does (front-running) | `__constructor` runs inside the deploy transaction, so no configurable window exists | `wasm_constructor_requires_the_admins_auth` deploys the built wasm and checks the config it ends up with |
| Re-running configuration to replace the admin, token or deadline | The host refuses to invoke any `__`-prefixed function directly, so `__constructor` runs only once | `constructor_cannot_be_invoked_again` |
| Deploying a vault that names an admin who did not sign | `__constructor`: `admin.require_auth()` | `wasm_constructor_requires_the_admins_auth`, `wasm_constructor_records_the_admin_as_the_authorizer` (both run against the built wasm; see below) |
| A non-admin calling `fund`, `register`, `cancel` or `withdraw` | each: `config.admin.require_auth()` | `admin_functions_require_auth`, `non_admin_cannot_call_admin_functions`, `register_records_the_admin_as_the_authorizer` |
| A vault created already past its deadline | `__constructor`: `deadline <= env.ledger().timestamp()` → `InvalidDeadline`, which the deployer sees as a generic error (see below) | `constructor_rejects_a_deadline_in_the_past_or_present` |
| A function trapping if the config were ever missing | `storage::config` returns `NotInitialized`. Defensive only: see below | `every_function_reports_not_initialized_without_config` |

**Testing the constructor's authorization.** The unit-test helper
`env.register` approves a constructor's `require_auth` automatically, so it
cannot show that the admin must sign. The two `wasm_*` tests in
[`src/test_wasm.rs`](../contracts/disbursement-vault/src/test_wasm.rs) upload
the wasm that `stellar contract build` produced and deploy it through the
host's real deploy path. They are `#[ignore]`d in a plain `cargo test`, because
the wasm must be built first, and CI runs them right after the build.

**Constructor errors.** The host reports any error from a constructor to the
deployer as the generic `Error(Context, InvalidAction)`. The vault's own code,
such as `InvalidDeadline` (4), appears only in the diagnostic events.
`constructor_rejects_a_deadline_in_the_past_or_present` asserts both.

### Funds owed to recipients

| Threat | Function and guard | Test |
|---|---|---|
| Admin withdrawing tokens that back a pending allocation | `withdraw`: `amount > balance - total_allocated` → `InsufficientFunds` | `withdraw_cannot_touch_a_pending_allocation`, `the_backing_invariant_holds_throughout` |
| Registering allocations the balance cannot cover | `register`: `total_allocated > balance` → `InsufficientFunds` | `register_cannot_exceed_the_vault_balance` |
| Admin cancelling a pending allocation before the deadline | `cancel`: `timestamp < deadline` → `DeadlineNotReached` | `cancel_before_the_deadline_is_rejected` |
| Silently changing an existing allocation | `register`: `storage::allocation(&env, &recipient).is_some()` → `AllocationExists` | `register_rejects_a_duplicate_recipient` |
| Zero or negative amounts | `fund`, `register`, `withdraw`: `amount <= 0` → `InvalidAmount` | `fund_rejects_non_positive_amounts`, `register_rejects_non_positive_amounts`, `withdraw_rejects_non_positive_amounts` |

### Claims

| Threat | Function and guard | Test |
|---|---|---|
| Claiming someone else's allocation | `claim`: `recipient.require_auth()` | `claim_requires_the_recipients_own_auth`, `claim_records_the_recipient_as_the_authorizer` |
| Redirecting a payout | `claim` has no destination parameter; it transfers to `recipient` | `claim_pays_the_recipient` |
| Claiming twice | `claim`: status `Claimed` → `AlreadyClaimed` | `claim_twice_is_rejected` |
| Claiming after a cancel | `claim`: `timestamp >= deadline` → `DeadlinePassed`, checked before the status | `claim_after_cancel_is_rejected` |
| Claim and cancel both valid at the same moment | `claim` needs `timestamp < deadline`; `cancel` needs `timestamp >= deadline`. The deadline itself belongs to `cancel` | `the_deadline_boundary_belongs_to_cancel`, `claim_is_rejected_at_and_after_the_deadline` |
| Registering after the deadline, when the allocation could never be claimed | `register`: `timestamp >= deadline` → `DeadlinePassed` | `register_is_rejected_at_and_after_the_deadline` |
| Cancelling a claimed allocation, or cancelling twice, which would subtract it from `total_allocated` again | `cancel`: status `Claimed` → `AlreadyClaimed`; `Cancelled` → `AllocationCancelled` | `cancel_of_a_claimed_allocation_is_rejected`, `cancel_twice_is_rejected` |
| Re-entering `claim` through the token during the transfer | `claim` marks the allocation `Claimed` and updates the counters before calling `transfer` | **No test.** A Stellar Asset Contract does not call back into the vault, and a malicious token is out of scope |

### Arithmetic

| Threat | Function and guard | Test |
|---|---|---|
| Counter overflow trapping, or wrapping | `checked_add` and `checked_sub` return `MathOverflow` instead | `counters_use_checked_math` covers the `register` path. The checked operations in `claim`, `cancel`, `withdraw` and `available` have **no test** |

### Storage lifetime

| Threat | Function and guard | Test |
|---|---|---|
| An active vault's instance, or an allocation in use, being archived | `storage::extend_instance` in every state-changing call; `storage::extend_allocation` in `register`, `claim` and `cancel`. Each extends to 60 days once 30 days or fewer remain | `state_changing_calls_extend_the_instance_ttl`, `register_claim_and_cancel_extend_the_allocation_ttl`, `extension_applies_at_the_30_day_threshold_and_not_above` |
| Third-party reads paying to extend entries | Getters call no `extend_*` | `getters_do_not_extend_ttl` |

## Defensive code: the `Cancelled` arm in `claim`

`claim` matches `AllocationStatus::Cancelled => return Err(Error::AllocationCancelled)`.
On a real network that arm cannot be reached:

1. `cancel` only succeeds when `timestamp >= deadline`.
2. The deadline is fixed by the constructor and nothing changes it.
3. Ledger time does not go backwards, so once an allocation is cancelled, every
   later ledger also has `timestamp >= deadline`.
4. `claim` checks `timestamp >= deadline` → `DeadlinePassed` **before** it
   reads the allocation's status.

So a claim on a cancelled allocation always fails with `DeadlinePassed`
(`claim_after_cancel_is_rejected`), and coverage reports this arm as the one
line in `lib.rs` that never runs.

**It stays deliberately.** It keeps the `match` exhaustive without a wildcard,
so a future status gets a compile error rather than silently falling through.
It also keeps `claim` safe on its own if the deadline check is ever reordered
or relaxed.

## Defensive code: `NotInitialized`

Every function except the constructor loads the config through
`storage::config`, which returns `NotInitialized` if it is absent. On a
deployed v0.1.0 vault it is never absent: the constructor writes it in the
deploy transaction, and nothing removes it. The typed error stays so that a
missing config could never become a trap, and
`every_function_reports_not_initialized_without_config` reaches it by removing
the config by hand. `AlreadyInitialized` (1) is no longer returned at all. Both
codes stay defined because error codes are stable, and a legacy v0.0.0 vault
still returns them.

## What the contract does not prevent

| Limitation | Effect | Mitigation |
|---|---|---|
| **The legacy v0.0.0 vault can be front-run at initialization** | v0.0.0 deploys and runs `initialize` in separate transactions, so whoever calls `initialize` first becomes admin. Fixed in v0.1.0 by the constructor | Use v0.1.0. On a v0.0.0 vault, check `get_admin`, `get_token` and `get_deadline` before funding it |
| **A compromised admin key** | The holder can withdraw unencumbered funds, register any addresses (its own included) against them, and cancel and reclaim all unclaimed allocations after the deadline. It cannot take pending funds before the deadline | Keep the admin key offline. There is no multisig and no way to rotate the admin |
| **A single, fixed admin** | No function changes the admin | Deploy a new vault |
| **No upgrade path** | No function replaces the contract code; a bug fix means a new deployment | Deploy a new vault and move the funds |
| **The token is not checked** | The constructor accepts any address as the token. A malicious or non-standard token can misreport balances, refuse transfers or re-enter | Use a Stellar Asset Contract or another audited SEP-41 token. Check `get_token` |
| **Recipients cannot be listed** | No function returns all allocations; recipients are known only from `registered` events or off-chain records | Issue #8 tracks this. See [EVENTS.md](EVENTS.md) |
| **Unclaimed allocations can be reclaimed after the deadline** | A recipient who does not claim in time loses the allocation once the admin cancels | Intended. Recipients must claim before the deadline |
| **Archival when idle** | An entry not touched within its TTL (60 days after its last extension) is archived and must be restored before the vault can use it | Call the vault, or extend TTLs externally, before 60 days pass |
| **`total_claimed` can in theory overflow** | `total_claimed` only grows. If tokens are cycled through the vault until cumulative claims exceed `i128::MAX`, `claim` returns `MathOverflow` and that allocation can only be cancelled after the deadline | None in the contract. Needs a token supply near `i128::MAX`. **No test** |

## Out of scope

- Bugs in `soroban-sdk`, the Soroban host, the Stellar network or the token.
- The backend indexer, API and web UI (separate repositories).
- An admin acting within the documented rules.
- Mainnet deployment: the contract is not audited and is testnet only.
