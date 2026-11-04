# Contributing

Thanks for your interest. This is a small Soroban contract repository; the
conventions below exist so that review stays fast and `main` stays releasable.

Security problems are the one exception to everything here — **do not** open a
public issue or PR for them. Follow [SECURITY.md](SECURITY.md) instead.

## Before you write code: get the issue assigned

**Every change starts from an issue, and the issue must be assigned to you
before you start.**

1. Find an open issue, or open one ([bug report](.github/ISSUE_TEMPLATE/bug_report.yml)
   or [task](.github/ISSUE_TEMPLATE/task.yml)).
2. Comment asking for it, and wait to be assigned.
3. Only then start work, and reference the issue from your PR.

This is not bureaucracy for its own sake: it prevents two people silently
building the same thing, and it is where the *approach* gets agreed. A PR that
changes the contract interface, the storage layout or the error codes is much
harder to redirect once written, so the design conversation belongs on the
issue first.

**Unsolicited PRs with no assigned issue may be closed without review.** If
you have already written the code, say so in the issue — nobody will ask you
to throw it away.

Typo and documentation fixes are exempt. Open the PR directly.

## Toolchain

| Tool | Version |
|---|---|
| Rust | **1.95.0** — CI pins this exact version; `soroban-sdk` 29 needs ≥ 1.91 |
| Target | `wasm32v1-none` |
| [stellar-cli](https://github.com/stellar/stellar-cli) | **26.0.0** — the version the contract is verified against |

```sh
rustup toolchain install 1.95.0
rustup target add wasm32v1-none --toolchain 1.95.0
rustup component add rustfmt clippy --toolchain 1.95.0
```

The Rust version is pinned so that a new stable release cannot turn a green PR
red without a deliberate bump. If you need a newer compiler, change the pin in
[.github/workflows/ci.yml](.github/workflows/ci.yml) in its own commit and say
why.

## Build and test

Run all five locally before you push. These are exactly what CI runs, in the
same order, so a clean run here means a green PR:

```sh
cargo fmt --all --check                          # formatting
cargo clippy --all-targets --all-features -- -D warnings   # lints, warnings are errors
cargo test --all-features                        # 45 tests
cargo audit                                      # dependency advisories
stellar contract build                           # the deployable wasm
```

Notes:

- **`-D warnings` means a warning fails the build.** Fix it rather than
  `#[allow]`-ing it; if an `allow` is genuinely right, comment why.
- `cargo fmt --all --check` only reports. Use `cargo fmt --all` to apply.
- `cargo test` regenerates `test_snapshots/`. That directory is ignored — do
  not add snapshot churn to your diff.
- `cargo audit` has one declared exception in [.cargo/audit.toml](.cargo/audit.toml),
  with its justification. Add a new one only with a comment explaining why the
  advisory cannot reach the deployed wasm.
- `stellar contract build` must succeed, because `cargo test` compiles for the
  host and can pass while the wasm target fails.

CI additionally prints the wasm interface and size on every PR. Check that
diff if you did not intend an interface change.

## Tests are required

**A PR that changes contract behaviour must include tests, in the same PR.**
It will not be merged without them. Tests live in
[contracts/disbursement-vault/src/test.rs](contracts/disbursement-vault/src/test.rs).

What "adequate" means here:

| Change | Expected test |
|---|---|
| New entry point | Success path, plus every error it can return |
| New error variant | A test asserting that exact variant, not just "it failed" |
| Bug fix | A test that fails before the fix and passes after |
| Auth change | That the wrong caller is rejected — `register_records_the_admin_as_the_authorizer` is the pattern |
| Anything touching balances | That the backing invariant `total_allocated <= token.balance(contract)` still holds |

Assert on the specific error, not merely that the call failed — a test that
accepts any failure will keep passing after the contract starts failing for a
different reason. Follow the existing naming style: a sentence describing the
behaviour, like `withdraw_cannot_touch_a_pending_allocation`.

Documentation-only PRs need no tests.

## Branches

Branch from `main`, and name it `<type>/<short-kebab-summary>` using the same
types as commits:

```
feat/claim-batching
fix/deadline-off-by-one
docs/repo-health
```

`main` is protected: it takes no direct pushes, and both CI checks must pass
before a PR can merge. Never force-push a branch someone else is reviewing or
building on.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):

```
<type>(<optional scope>): <imperative summary>

<body: why, not what>
```

Types in use here, with real examples from this repository:

| Type | Example |
|---|---|
| `feat` | `feat(vault): implement disbursement vault contract` |
| `fix` | `fix(vault): reject a zero-amount registration` |
| `test` | `test(vault): cover every branch and error variant` |
| `docs` | `docs: add disbursement vault design document` |
| `ci` | `ci: run quality gates on every pull request` |
| `chore` | `chore(deps): upgrade soroban-sdk 25 -> 29.0.1` |

Scopes seen so far: `vault`, `bindings`, `deps`, `audit`.

- Imperative mood: "add", not "added" or "adds".
- Keep the subject under ~72 characters and do not end it with a period.
- **Prefer several small commits over one large one.** One commit should do one
  thing, so it can be read, reverted or bisected on its own.
- Use the body for *why*. The diff already says what changed; what a reviewer
  cannot reconstruct is the reasoning and the alternatives you rejected.
- Mark a breaking interface change with `!` — `feat(vault)!: ...` — and explain
  the migration in the body. Error codes are append-only and must never be
  renumbered.

## Pull requests

Fill in the [template](.github/pull_request_template.md). Keep the description
current as you push changes — a stale description is worse than a short one.

To merge:

- Both CI checks (**fmt, clippy, test, build** and **cargo audit**) green on
  the latest commit. A PR with a failing or still-running check is not merged.
- The branch up to date with `main`.
- No unresolved review comments.
- Linked to its issue.

Merges are **squash merges** with a conventional-commit title, so one PR
becomes one commit on `main`. Delete the branch afterwards. Keep your PR small
enough to review in one sitting; if it has grown, split it.

## Never commit secrets

No keys, seeds, secrets or mnemonics in code, tests, fixtures, logs, commit
messages or PR text. Not even expired or testnet ones — a key that looks real
costs a reviewer time to rule out, and habits transfer to mainnet keys.

Signing goes through the stellar-cli identity store, referenced by name via
`--source`. For a test fixture that needs a key-shaped string, build an
obviously invalid one (e.g. `"S" + "A".repeat(55)`) rather than pasting a real
keypair.

Secret scanning and push protection are enabled on this repository, but treat
them as a safety net, not a check.

## Layout

```text
contracts/disbursement-vault/src/
├── lib.rs       # entry points
├── types.rs     # Config, Allocation, AllocationStatus, DataKey
├── error.rs     # #[contracterror] enum — append-only
├── events.rs    # #[contractevent] structs
├── storage.rs   # storage accessors and TTL policy
└── test.rs      # the test suite
```

`bindings/disbursement-vault/` is **generated** by
`stellar contract bindings typescript`. Do not hand-edit it; change the
contract interface and regenerate, in the same PR.

Design rationale — storage layout, error taxonomy, TTL strategy, and the
boundary decisions — is in [docs/DESIGN.md](docs/DESIGN.md). Update it in the
same PR when you change one of those.
