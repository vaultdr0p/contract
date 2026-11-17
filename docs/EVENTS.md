# Events

The vault emits six contract events. This is the schema an indexer can rely on.
For why each event exists, see [DESIGN.md §4](DESIGN.md#4-events).

Every encoding below was checked against two sources:

- **The emitted events** in the Soroban test snapshots that `cargo test` writes
  to `contracts/disbursement-vault/test_snapshots/` (git-ignored, regenerated
  on every run).
- **The event spec in the wasm,** from `stellar contract info interface --wasm
  target/wasm32v1-none/release/disbursement_vault.wasm --output json-formatted`
  (the `event_v0` entries).

An event published from a constructor was also checked on testnet. A
throwaway contract with the same `Initialized` struct and the same SDK
published it from `__constructor`, and RPC `getEvents` returned it with the
same topics, value types and key order as an `initialized` event from the
earlier `initialize`-based contract. The deployed v0.1.0 vault's own
`initialized` event, in its deploy transaction at ledger 5,778,180, has the
same shape.

## Common shape

All six events have the same structure:

| Part | Content |
|---|---|
| Event type | `contract`, with `contract_id` set to the vault |
| `topics[0]` | `Symbol`: the event name, in snake case (`claimed`, …) |
| `topics[1]` | `Address`: the one party the event is about |
| `data` | `Map` of `Symbol` keys to values |

Two rules follow from the encoding:

- **Read `data` by key, not by position.** An `ScMap` is ordered by key, not by
  field declaration. For example, `Initialized` declares `token` then
  `deadline`, but the emitted map lists `deadline` first.
- **Amounts are `i128`** in the token's smallest unit, and `deadline` is `u64`
  unix seconds. Use 128-bit or decimal types; amounts do not fit in a
  JavaScript `number`.

An `Address` can be an account (`G…`) or a contract (`C…`).

## The events

| Event | Emitted by | `topics[1]` | `data` keys and types |
|---|---|---|---|
| `initialized` | the constructor, in the deploy transaction | `admin: Address` | `deadline: u64`, `token: Address` |
| `funded` | `fund` | `from: Address` (the admin) | `amount: i128`, `balance: i128` |
| `registered` | `register` | `recipient: Address` | `amount: i128`, `total_allocated: i128` |
| `claimed` | `claim` | `recipient: Address` | `amount: i128` |
| `cancelled` | `cancel` | `recipient: Address` | `amount: i128` |
| `withdrawn` | `withdraw` | `to: Address` | `amount: i128` |

Field meanings:

- **`funded.balance`** is the vault's token balance read *after* the transfer.
  It includes any tokens sent to the vault directly with the token's own
  `transfer`, which emit no vault event. So `balance` can be larger than the
  sum of `funded.amount`.
- **`registered.total_allocated`** is the total pending encumbrance after this
  registration: the sum of all `Pending` allocations.
- **`claimed.amount`** and **`cancelled.amount`** are always the full
  allocation. There are no partial claims.
- **`withdrawn.to`** is chosen by the admin and can be any address.

### Example

The `claimed` event for a 300-unit claim, as the test snapshot records it:

```json
{
  "type": "contract",
  "contract_id": "C…",
  "body": { "v0": {
    "topics": [ { "symbol": "claimed" }, { "address": "C…" } ],
    "data": { "map": [ { "key": { "symbol": "amount" }, "val": { "i128": "300" } } ] }
  } }
}
```

## When events are emitted

- **Only on success.** In every entry point the event is published last, after
  all guards and state writes. A call that returns an error never reaches it.
  `a_rejected_call_emits_no_event` tests this for `register`.
- **Exactly one vault event per successful call.** `every_operation_emits_its_event`
  asserts each call's vault events are exactly the expected one.
- **Read-only functions emit nothing.**

### Order relative to the token transfer

Three calls move tokens. In each, the token's own `transfer` event comes
**before** the vault event in the same transaction:

| Call | Events, in order |
|---|---|
| `fund` | token `transfer` (admin → vault), then `funded` |
| `claim` | token `transfer` (vault → recipient), then `claimed` |
| `withdraw` | token `transfer` (vault → `to`), then `withdrawn` |

The constructor, `register` and `cancel` move no tokens and emit only the
vault event.

## Reconstructing state from events

These follow from the code and the events above:

- **Configuration:** `initialized` gives the admin, token and deadline. None of
  them can change afterwards. Since v0.1.0 it is in the same transaction, and
  so the same ledger, as the contract's creation.
- **Allocations:** each recipient has at most one `registered`. After that it
  gets at most one of `claimed` or `cancelled`, never both. No event means it is
  still pending.
- **`total_allocated`:** the latest `registered.total_allocated`, minus the
  `amount` of every later `claimed` and `cancelled`.
- **`total_claimed`:** the sum of `claimed.amount`.
- **Balance:** cannot be derived from vault events alone, because direct token
  transfers to the vault emit none. Use the latest `funded.balance` as a
  checkpoint, or read the token's `balance` for the vault directly. The vault's
  `available` getter returns that balance minus `total_allocated`.

### Limits for indexers

- **Events are not kept forever.** Soroban RPC keeps events only for a limited
  retention window. An indexer that starts late cannot replay from
  `initialized` and has to read current state from the contract's getters
  instead.
- **There is no list of recipients.** The contract cannot enumerate
  allocations, so the event stream is the only way to discover recipients.
  Tracked in issue #8.
