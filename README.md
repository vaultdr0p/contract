# Disbursement Vault

A Soroban contract for programmatic disbursements. An admin funds a vault with
a single token (a Stellar Asset Contract such as USDC), registers recipients
with fixed allocations, and each recipient claims their own. After a deadline
the admin can cancel unclaimed allocations and reclaim the funds.

The design rationale — storage layout, error taxonomy, TTL strategy and the
boundary decisions — is in [docs/DESIGN.md](docs/DESIGN.md).

## Deployment

### Testnet

| | |
|---|---|
| **Contract ID** | `CB5HTYC4GZW67ET23SACDCWCCDQ3AKU4TYX7SAUQJSKS6LLQLYA4HG53` |
| **Wasm hash** | `d34c4fc683959bcfe8a9cb41122e04ffd36cb04d6c3f08c385e35078c32ebcd0` |
| **Network** | `Test SDF Network ; September 2015` |
| **Explorer** | [stellar.expert](https://stellar.expert/explorer/testnet/contract/CB5HTYC4GZW67ET23SACDCWCCDQ3AKU4TYX7SAUQJSKS6LLQLYA4HG53) |
| **Token** | native XLM SAC (`CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`) |

This instance is **already initialized** and exists for interface verification.
Deploy your own for anything real — `initialize` can only be called once, so the
admin and deadline on this one are fixed.

Mainnet: not deployed.

## Safety invariant

Every registered-but-unclaimed allocation is fully backed by tokens the contract
actually holds:

```
total_allocated <= token.balance(contract)
```

`withdraw` is capped at `balance - total_allocated`, so the admin can never
withdraw funds owed to a recipient who has not yet claimed. This is the
contract's central safety property and is covered directly by
`withdraw_cannot_touch_a_pending_allocation`.

## Interface

All amounts are `i128`. Every state-changing function calls `require_auth()`.

| Function | Auth | Effect |
|---|---|---|
| `initialize(admin, token, deadline)` | `admin` | Writes config. Callable once; `deadline` is unix seconds and must be in the future |
| `fund(amount)` | `admin` | Transfers `amount` from the admin into the vault |
| `register(recipient, amount)` | `admin` | Earmarks `amount`. Only before the deadline, only if it still fits in the balance |
| `claim(recipient)` | `recipient` | Pays the recipient. Only strictly before the deadline |
| `cancel(recipient)` | `admin` | Releases an unclaimed allocation. Only at or after the deadline. Moves no tokens |
| `withdraw(to, amount)` | `admin` | Sends unencumbered funds out, capped at `balance - total_allocated` |

Read-only, no auth: `get_admin()`, `get_token()`, `get_deadline()`,
`get_allocation(recipient)`, `total_allocated()`, `total_claimed()`,
`available()`.

The deadline is a hard cutover: `claim` requires `now < deadline` and `cancel`
requires `now >= deadline`, so exactly one of the two is ever valid and they
cannot race. "Reclaim" is `cancel` followed by `withdraw`.

## Error codes

| Code | Name | Raised by |
|---|---|---|
| 1 | `AlreadyInitialized` | second `initialize` |
| 2 | `NotInitialized` | any function before `initialize` |
| 3 | `InvalidAmount` | `amount <= 0` |
| 4 | `InvalidDeadline` | `initialize` with a deadline at or before now |
| 5 | `AllocationExists` | `register` for an address that already has one |
| 6 | `AllocationNotFound` | `claim`/`cancel` for an unregistered address |
| 7 | `AlreadyClaimed` | double `claim`, or `cancel` of a claimed allocation |
| 8 | `AllocationCancelled` | `claim` or `cancel` after a `cancel` |
| 9 | `InsufficientFunds` | `register` beyond the balance, or `withdraw` beyond the unencumbered balance |
| 10 | `DeadlineNotReached` | `cancel` before the deadline |
| 11 | `DeadlinePassed` | `register`/`claim` at or after the deadline |
| 12 | `MathOverflow` | checked arithmetic over/underflowed |

Codes are stable: variants are only ever appended, never renumbered.

## Development

Requires Rust ≥ 1.91 (CI pins 1.95.0), the `wasm32v1-none` target, and
[stellar-cli](https://github.com/stellar/stellar-cli) 26.0.0.

```sh
rustup target add wasm32v1-none

cargo test                                 # 45 tests
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo audit
stellar contract build
```

All five run on every PR via [.github/workflows/ci.yml](.github/workflows/ci.yml).

`cargo audit` has one declared exception, justified in
[.cargo/audit.toml](.cargo/audit.toml): RUSTSEC-2026-0387 (`crypto-bigint`)
reaches the tree only through `soroban-env-host` under `testutils` and
dev-dependencies, and is absent from the wasm dependency graph.

## Deploying your own

```sh
stellar contract build

stellar contract deploy \
  --wasm target/wasm32v1-none/release/disbursement_vault.wasm \
  --source <identity> \
  --network testnet \
  --alias disbursement_vault

stellar contract invoke --id <contract-id> --source <identity> --network testnet -- \
  initialize \
    --admin <admin-address> \
    --token <token-sac-address> \
    --deadline <unix-seconds>
```

To get a token's SAC address: `stellar contract id asset --asset <CODE:ISSUER> --network testnet`
(or `--asset native` for XLM).

## Secrets and key handling

No keys, seeds or secrets live in this repository, and none are written to
logs. Signing is delegated entirely to the stellar-cli's own identity store
(`~/.config/stellar/identity`), referenced only by name via `--source`.

For CI or server use, supply keys through the environment and never commit
them:

| Variable | Purpose |
|---|---|
| `STELLAR_ACCOUNT` | identity name or address used as the transaction source |
| `STELLAR_SECRET_KEY` | signing secret, when not using a named identity |
| `STELLAR_NETWORK` | network name, e.g. `testnet` |
| `STELLAR_RPC_URL` | Soroban RPC endpoint |
| `STELLAR_NETWORK_PASSPHRASE` | network passphrase |

`.gitignore` excludes `.stellar/` and `.soroban/`, which can contain local
identity material.

## TypeScript bindings

Generated into [bindings/disbursement-vault](bindings/disbursement-vault) from
the deployed contract:

```sh
stellar contract bindings typescript \
  --contract-id <contract-id> \
  --network testnet \
  --output-dir bindings/disbursement-vault

cd bindings/disbursement-vault && npm install && npm run build
```

The generated client embeds the contract address it was generated against, and
exposes the typed `Client`, the `Allocation`/`AllocationStatus` types, and the
`Errors` map above. Regenerate after any interface change.

## Repository layout

```text
.
├── contracts/disbursement-vault/
│   └── src/
│       ├── lib.rs        # entry points
│       ├── types.rs      # Config, Allocation, AllocationStatus, DataKey
│       ├── error.rs      # typed #[contracterror] enum
│       ├── events.rs     # typed #[contractevent] structs
│       ├── storage.rs    # storage accessors and TTL policy
│       └── test.rs       # 45 tests
├── bindings/disbursement-vault/   # generated TypeScript client
├── docs/DESIGN.md
├── .cargo/audit.toml
└── .github/workflows/ci.yml
```
