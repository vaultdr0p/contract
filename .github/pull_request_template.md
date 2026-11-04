<!--
Security fix? Do not open a public PR. See SECURITY.md.
Conventions, including the five local gates, are in CONTRIBUTING.md.
-->

## What and why

<!-- What changes, and the reasoning a reviewer cannot get from the diff. -->

Closes #

## Checklist

- [ ] Linked to an **assigned** issue above (or this is a docs-only change)
- [ ] `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, `cargo audit` and `stellar contract build` all pass locally
- [ ] Behaviour changes have tests in this PR, asserting the **specific** error variant where one applies
- [ ] No keys, seeds or secrets in code, tests, fixtures, logs, commit messages or this description
- [ ] Conventional-commit title; `docs/DESIGN.md` updated if the design changed
- [ ] Interface change: bindings regenerated, error codes appended (never renumbered), and the migration explained above — or no interface change

## Notes for the reviewer

<!-- Anything they must decide, trade-offs taken, or what you deliberately left out. Delete if empty. -->
