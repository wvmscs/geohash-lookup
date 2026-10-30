# Contributing

Issues and pull requests are welcome. Code, comments and documentation are in English.

## Before a pull request

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --no-default-features --all-targets -- -D warnings
cargo test
cargo deny check advisories licenses sources
reuse lint
```

The CI runs the same checks on every pull request.

## Rules

- **Branch:** open the pull request against `main`, **rebased** on its latest commit
  (no merge commits). Rebase again when `main` moves.
- **Licence:** contributions are accepted under the licence of the project (GPL-3.0-or-later).
  Every new file starts with its SPDX tags (`SPDX-FileCopyrightText`,
  `SPDX-License-Identifier`), or is covered by `REUSE.toml`.
- **Dependencies:** a new dependency must pass `cargo deny` (licence compatible with
  GPL-3.0-or-later).
- **Tests are offline** and reproducible: no request to BAN, Nominatim or DB-IP from
  `cargo test`. Check the real services by hand.
- **Services:** respect the public Nominatim policy (one request a second, a User-Agent
  naming the caller, no bulk use).
- **No personal data** in issues, logs or test data: no private address, no person's IP
  address. Use public places and public resolvers.
