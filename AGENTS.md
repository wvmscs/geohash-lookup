# AGENTS.md

Guidance for coding agents (and humans) working in this crate.

## What this is

`geohash-lookup`: a place (address, town, country) or an IP address to a geohash cell, and a
cell back to an approximate place name. A library (async, Tokio) and a command-line tool.
It serves the cell fields and command lines of
[bitchat-console](https://github.com/wvmscs/bitchat-console) and
[bitchat-geohash](https://github.com/wvmscs/bitchat-geohash), behind their cargo feature
`geocode`. Users and options: [USAGE.md](USAGE.md); installation and releases:
[INSTALL.md](INSTALL.md); a skill for agents: [skills/geohash-lookup/SKILL.md](skills/geohash-lookup/SKILL.md).

Rules that only concern the author's local checkout, if any, are in `AGENTS.local.md`
(not committed).

## Constraints

- Sources: BAN (IGN Géoplateforme), Nominatim (OpenStreetMap), a local MMDB file (DB-IP City
  Lite, downloaded by `geoip`). No other service without discussion.
- Respect the public Nominatim policy: one request a second (`NOMINATIM_MIN_GAP`), a
  User-Agent naming the caller, no bulk use.
- Keep the data credits reachable (`Provider::attribution`, `geoip::DBIP_ATTRIBUTION`).
- Requests go straight to the services; the docs say so (privacy). Never log queries.
- Tests are offline: no network in `cargo test`. Real-service checks are by hand (INSTALL.md,
  section Releases).
- No personal data in tests, docs or examples (no private address, key, IP of a person):
  use public places (Eiffel Tower, Rue de Rivoli, town halls) and public resolvers (8.8.8.8).
- Code, comments, rustdoc and docs in English.
- Dependencies: permissive or GPL-compatible only (`cargo deny`); new ones after discussion.
- Own git repository (`origin` = `git@github.com:wvmscs/geohash-lookup.git`, branch `main`),
  own `[workspace]`. Consumers pin tags. Commits only when the user asks.

## Commands

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --no-default-features --all-targets -- -D warnings
cargo test
cargo deny check advisories licenses sources
reuse lint      # every file has its copyright and licence (also a GitHub Action)
```

<!-- REUSE-IgnoreStart -->
A new file starts with
`SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)` and
`SPDX-License-Identifier: GPL-3.0-or-later` in a comment, or is covered by `REUSE.toml`.
<!-- REUSE-IgnoreEnd -->
