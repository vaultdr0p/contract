# Security Policy

## Status: testnet only, and not audited

This contract has **not been audited**. It is deployed on Stellar **testnet
only**, where the tokens have no monetary value, and it is not deployed on
mainnet. Treat it as unreviewed software.

Do not use it to custody real funds. If you deploy it anywhere that holds
value, you are doing so against this advice and the risk is entirely yours —
see the warranty and liability disclaimers in [LICENSE](LICENSE).

## Reporting a vulnerability

**Report privately. Do not open a public issue for a security problem.**

Use GitHub's private vulnerability reporting:

**[→ Report a vulnerability](https://github.com/vaultdr0p/contract/security/advisories/new)**

(Also reachable from the repository's **Security** tab → **Report a
vulnerability**.) The report stays visible only to you and the maintainers
until an advisory is published.

GitHub's form is the only reporting channel this project offers. It is
preferred over email because it keeps the report, the discussion, the fix and
the eventual advisory attached to the repository, rather than in a mailbox
only one person can read.

### What to include

A report is easiest to act on when it has:

- which function or invariant is affected, and the commit or contract ID you tested
- the conditions required — who must be authorized, and whether the deadline has passed
- what an attacker gains: funds moved, an allocation claimed twice, a stuck vault
- a failing test or a `stellar contract invoke` sequence that reproduces it

A reproduction against `cargo test` is the most useful single thing you can
send, because it goes straight into the regression suite.

**Never include secret keys or seeds** in a report. Testnet addresses and
transaction hashes are fine.

## Scope

| In scope | |
|---|---|
| `contracts/disbursement-vault/` | the contract source — auth, arithmetic, storage, the deadline cutover |
| The safety invariant | any way to make `total_allocated > token.balance(contract)`, or to withdraw funds backing an unclaimed allocation |
| `Cargo.toml` / `Cargo.lock` | a dependency or build setting that changes the deployed wasm |
| `.github/workflows/` | anything letting untrusted input run with write access to this repo |

| Out of scope | Where it belongs |
|---|---|
| The deployed testnet instance's state or balances | Not a vulnerability: it is a throwaway, already-initialized demo holding valueless tokens |
| Bugs in `soroban-sdk`, the Stellar host, or other dependencies | Report upstream. Tell us too, if this contract is exploitable because of it |
| The indexer, API and web UI | The `vaultdr0p/backend` and `vaultdr0p/frontend` repositories |
| Generated TypeScript in `bindings/` | Regenerated output. Report the contract interface problem behind it |
| Missing mainnet hardening, no audit, no multisig admin | Known and documented above, not separate findings |

The admin is trusted by design. An admin funding, registering, cancelling or
withdrawing within the documented rules is the contract working as intended —
including reclaiming unclaimed allocations after the deadline. A report is only
in scope if it lets a **non-admin** act as one, or lets the admin break the
backing invariant.

## Response times

Maintained by volunteers, so these are deliberately conservative — they are
what can be met, not a best case:

| Stage | Target |
|---|---|
| Acknowledgement that a human has read it | **5 business days** |
| Initial assessment — in scope or not, and rough severity | **10 business days** |
| Fix or a documented decision not to fix | **30 business days** for something exploitable in scope |

If 10 business days pass with no acknowledgement, the report was probably
missed. Open a public issue containing **no details** — just that you are
waiting on a security contact — and that will be treated as a priority.

Since nothing is deployed on mainnet, a fix here means a merged patch and a
redeployed testnet instance; there is no emergency upgrade path to coordinate
and no funds at risk while a fix is pending.

## Disclosure

Coordinated disclosure. We will agree a publication date with you, defaulting
to **the earlier of: the fix landing on `main`, or 90 days** from the
acknowledgement. Reporters are credited in the advisory by whatever name they
ask for, or not at all if they prefer.

There is **no bug bounty** and no payment. Reports are still genuinely
welcome, and this policy exists so you know what happens to one.
