# Disbursement Vault

A Soroban contract for programmatic disbursements. An admin funds a vault with
a single token (a Stellar Asset Contract such as USDC), registers recipients
with fixed allocations, and each recipient claims their own. After a deadline
the admin can cancel unclaimed allocations and reclaim the funds.

The design rationale — storage layout, error taxonomy, TTL strategy and the
boundary decisions — is in [docs/DESIGN.md](docs/DESIGN.md). What the
contract defends against, and what it does not, is in
[docs/THREAT_MODEL.md](docs/THREAT_MODEL.md). The event schema for indexers is
in [docs/EVENTS.md](docs/EVENTS.md).

## Status

> **Testnet only, and not audited.**
>
> This contract has had **no external security audit**. It is deployed on
> Stellar **testnet**, where tokens have no monetary value, and it is **not
> deployed on mainnet**.
>
> **Do not use it to custody real funds.** It is published for review and
> interface verification. If you deploy it somewhere that holds value, that is
> against this advice and the risk is yours — see the warranty and liability
> disclaimers in [LICENSE](LICENSE).

What *is* true: the safety invariant below is covered by tests, every entry
point and every error the contract can still return is exercised by the test
suite (`cargo test`), and six quality gates run on every pull request. None of that substitutes for an audit.

Found a security problem? Report it privately — see
[SECURITY.md](SECURITY.md). Want to contribute? Start with
[CONTRIBUTING.md](CONTRIBUTING.md) and our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Deployment

### Testnet: v0.1.0 (current)

| | |
|---|---|
| **Contract ID** | `CDJGWM7I3L4MJJ673AW2O5GX35Y7Z2DKPMGE4HWZFIMXMTI7KECWF2TN` |
| **Wasm hash** | `0e67cc77f309e6d0fde1f80ce651399672b5811a619f9a66da08dafcb1960bc7` |
| **Network** | `Test SDF Network ; September 2015` |
| **Explorer** | [stellar.expert](https://stellar.expert/explorer/testnet/contract/CDJGWM7I3L4MJJ673AW2O5GX35Y7Z2DKPMGE4HWZFIMXMTI7KECWF2TN) |
| **Token** | native XLM SAC (`CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`) |
| **Admin** | `GAODBHVR63Z56MVQRBEJSYM2H5423LJ4WAPUUBOFG4JYY72S6ROKVZRX` |
| **Deadline** | `1802690514` (2027-02-15 11:21:54 UTC) |

Built from commit `9c7b3d4` with `stellar contract build` (stellar-cli 26.0.0,
Rust 1.95.0). To check that the deployed code matches the source, build that
commit and compare:

```sh
sha256sum target/wasm32v1-none/release/disbursement_vault.wasm
stellar contract fetch --id CDJGWM7I3L4MJJ673AW2O5GX35Y7Z2DKPMGE4HWZFIMXMTI7KECWF2TN --network testnet -o deployed.wasm && sha256sum deployed.wasm
```

Both should print the wasm hash above.

### Testnet: v0.0.0 (legacy)

**Legacy.** This instance runs the v0.0.0 interface, which has `initialize`
instead of a constructor. It stays on testnet, because the contract has no
upgrade path, but it is not the current version and receives no fixes.

| | |
|---|---|
| **Contract ID** | `CB5HTYC4GZW67ET23SACDCWCCDQ3AKU4TYX7SAUQJSKS6LLQLYA4HG53` |
| **Wasm hash** | `d34c4fc683959bcfe8a9cb41122e04ffd36cb04d6c3f08c385e35078c32ebcd0` |
| **Network** | `Test SDF Network ; September 2015` |
| **Explorer** | [stellar.expert](https://stellar.expert/explorer/testnet/contract/CB5HTYC4GZW67ET23SACDCWCCDQ3AKU4TYX7SAUQJSKS6LLQLYA4HG53) |
| **Token** | native XLM SAC (`CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`) |

It is already initialized, so its admin and deadline are fixed.

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
| `__constructor(admin, token, deadline)` | `admin` | Writes config, in the deploy transaction. Runs once and cannot be called afterwards. `deadline` is unix seconds and must be in the future |
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
| 1 | `AlreadyInitialized` | not returned since v0.1.0 (v0.0.0: a second `initialize`) |
| 2 | `NotInitialized` | not returned by a deployed v0.1.0 vault, which always has a config (v0.0.0: any function before `initialize`) |
| 3 | `InvalidAmount` | `amount <= 0` |
| 4 | `InvalidDeadline` | the constructor, with a deadline at or before now. The deployer sees a generic `Error(Context, InvalidAction)`; code 4 appears only in the diagnostic events |
| 5 | `AllocationExists` | `register` for an address that already has one |
| 6 | `AllocationNotFound` | `claim`/`cancel` for an unregistered address |
| 7 | `AlreadyClaimed` | double `claim`, or `cancel` of a claimed allocation |
| 8 | `AllocationCancelled` | `cancel` after a `cancel`. (`claim` after a `cancel` fails earlier, with `DeadlinePassed`) |
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

cargo test                                 # the test suite
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo audit
stellar contract build
cargo test -- --ignored                    # tests against the built wasm
```

All six run on every PR via [.github/workflows/ci.yml](.github/workflows/ci.yml).

`cargo audit` has one declared exception, justified in
[.cargo/audit.toml](.cargo/audit.toml): RUSTSEC-2026-0387 (`crypto-bigint`)
reaches the tree only through `soroban-env-host` under `testutils` and
dev-dependencies, and is absent from the wasm dependency graph.

## Deploying your own

```sh
stellar contract build

stellar contract deploy \
  --wasm target/wasm32v1-none/release/disbursement_vault.wasm \
  --source <admin-identity> \
  --network testnet \
  --alias disbursement_vault \
  -- \
  --admin <admin-address> \
  --token <token-sac-address> \
  --deadline <unix-seconds>
```

The arguments after `--` go to the constructor, which runs in the same
transaction as the deploy, so no one can configure the vault before you do.
The constructor requires the admin's authorization; deploying with the admin
as `--source` provides it.

Then check the configuration:

```sh
stellar contract invoke --id <contract-id> --network testnet --source <identity> --send=no -- get_admin
stellar contract invoke --id <contract-id> --network testnet --source <identity> --send=no -- get_token
stellar contract invoke --id <contract-id> --network testnet --source <identity> --send=no -- get_deadline
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
│   ├── Cargo.toml                  # crate manifest
│   └── src/
│       ├── lib.rs                  # entry points
│       ├── types.rs                # Config, Allocation, AllocationStatus, DataKey
│       ├── error.rs                # typed #[contracterror] enum — append-only
│       ├── events.rs               # typed #[contractevent] structs
│       ├── storage.rs              # storage accessors and TTL policy
│       ├── test.rs                 # the test suite
│       └── test_wasm.rs            # tests against the built wasm (run with --ignored)
├── bindings/disbursement-vault/    # generated TypeScript client
├── docs/
│   ├── DESIGN.md                   # design rationale and boundary decisions
│   ├── EVENTS.md                   # event schema and encoding for indexers
│   └── THREAT_MODEL.md             # what the contract prevents, with tests
├── .github/
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.yml          # bug form
│   │   ├── task.yml                # task form
│   │   └── config.yml              # blank issues off, links private reporting
│   ├── pull_request_template.md    # review checklist
│   └── workflows/ci.yml            # fmt, clippy, test, audit, wasm build
├── .cargo/audit.toml               # declared cargo audit exceptions
├── Cargo.toml                      # workspace manifest and release profile
├── Cargo.lock
├── .gitignore
├── README.md
├── CHANGELOG.md                    # versions and breaking changes
├── LICENSE                         # Apache-2.0
├── SECURITY.md                     # how to report a vulnerability privately
├── CONTRIBUTING.md                 # gates, conventions, issue assignment
└── CODE_OF_CONDUCT.md              # Contributor Covenant 2.1
```

## License

[Apache-2.0](LICENSE). Copyright 2026 vaultdr0p contributors.

Note the disclaimers in sections 7 and 8: the software is provided "as is",
without warranties or conditions of any kind, and the contributors are not
liable for damages arising from its use. Given that it is unaudited and
testnet-only, read those as meant literally.
