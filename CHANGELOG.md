# Changelog

All notable changes to the disbursement vault contract. Versions follow
[Semantic Versioning](https://semver.org/); before 1.0.0, a breaking change
bumps the minor version.

## 0.1.0 (2026-11-17)

### Breaking

- **`initialize` is replaced by a constructor.** `__constructor(admin, token,
  deadline)` runs in the same transaction as the deploy. This closes the window
  in which anyone could call `initialize` on a freshly deployed vault and make
  themselves admin. Constructor arguments go after `--` in `stellar contract
  deploy`; see the README.
- **A new deployment is required.** The contract has no upgrade path, so an
  existing v0.0.0 vault cannot become v0.1.0. Deploy a new vault, which gets a
  new contract ID, and point clients at it. Regenerate the TypeScript
  bindings: `initialize` is gone, and `deploy` takes the constructor arguments.
- **Error reporting at deploy.** An invalid deadline still fails with
  `InvalidDeadline` (4) inside the contract, but the host reports a failed
  constructor to the deployer as `Error(Context, InvalidAction)`. Code 4
  appears only in the diagnostic events.

### Unchanged

- Every other function, its arguments, and its return type.
- All six events and their encoding. `initialized` is now emitted by the
  constructor, inside the deploy transaction.
- Error codes: none removed or renumbered. `AlreadyInitialized` (1) is no
  longer returned, and `NotInitialized` (2) is only defensive on a deployed
  v0.1.0 vault. A legacy vault can still return both.

### Added

- Tests that deploy the built wasm through the host's real deploy path, to
  prove the constructor requires the admin's authorization
  (`src/test_wasm.rs`). CI runs them after `stellar contract build`.

## 0.0.0

The first deployed version, configured with a separate `initialize` call.
Legacy: its testnet instance
`CB5HTYC4GZW67ET23SACDCWCCDQ3AKU4TYX7SAUQJSKS6LLQLYA4HG53` (wasm hash
`d34c4fc683959bcfe8a9cb41122e04ffd36cb04d6c3f08c385e35078c32ebcd0`) stays
deployed but is no longer supported.
