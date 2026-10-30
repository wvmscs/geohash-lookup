# Installation

For macOS and Linux. No system OpenSSL: TLS is `rustls` (with `ring`), certificates are those
of the system (`rustls-platform-verifier`).

1. [Requirements](#1-requirements)
2. [Install the command](#2-install-the-command)
3. [Use the library](#3-use-the-library)
4. [GeoIP file](#4-geoip-file)
5. [Name yourself to Nominatim](#5-name-yourself-to-nominatim)
6. [Self-hosted Nominatim](#6-self-hosted-nominatim-optional)
7. [Releases (maintainers)](#7-releases-maintainers)
8. [Troubleshooting](#8-troubleshooting)

## 1. Requirements

| What | Detail |
|---|---|
| Rust **>= 1.88** | `rustc --version`; <https://rustup.rs> |
| C compiler and linker | for `ring` (rustls' crypto) |
| Outgoing HTTPS | to `data.geopf.fr` (BAN), `nominatim.openstreetmap.org` (or yours), `download.db-ip.com` (GeoIP download) |

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
```

C compiler: `xcode-select --install` (macOS), `sudo apt install build-essential` (Debian,
Ubuntu), `sudo dnf groupinstall "Development Tools"` (Fedora), `apk add build-base` (Alpine).
Linux certificates: the `ca-certificates` package (present on most systems; install it in
slim Docker images).

## 2. Install the command

From a release tag, into `~/.cargo/bin`:

```sh
cargo install --git https://github.com/wvmscs/geohash-lookup --tag v0.1.1 --locked
geohash-lookup --version
```

Or from a checkout:

```sh
git clone https://github.com/wvmscs/geohash-lookup && cd geohash-lookup
cargo test --locked                 # offline tests
cargo build --release --locked      # target/release/geohash-lookup
sudo install -m 0755 target/release/geohash-lookup /usr/local/bin/
```

Uninstall: `cargo uninstall geohash-lookup`, or delete the file.

## 3. Use the library

In the host's `Cargo.toml`, without the command line (no `clap`), pinned to a tag:

```toml
[dependencies]
geohash-lookup = { git = "https://github.com/wvmscs/geohash-lookup", tag = "v0.1.1", default-features = false }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

The API is async and needs a Tokio runtime (see [USAGE.md](USAGE.md)). Building a
`Geocoder` installs `ring` as the process-wide rustls crypto provider if none is installed; a
host that installs another one first keeps it. The bitchat tools take it behind a cargo
feature (`geocode`).

## 4. GeoIP file

Only for IP addresses. A **City** file in **MMDB** format, IPv4 and IPv6.

**Automatic**: give a path that does not exist yet, the tool downloads DB-IP's free
"IP to City Lite" of the month there (about 30 MB to download, 130 MB on disk and in memory),
checks it opens, then uses it:

```sh
geohash-lookup --geoip-db ~/.local/share/geohash-lookup/dbip-city-lite.mmdb 8.8.8.8
```

`--no-download` fails instead. From the library: `geohash_lookup::geoip::ensure(path, ua)`.
DB-IP updates the file monthly: delete it (or call `geoip::download`) to refresh it.

**By hand**: <https://db-ip.com/db/download/ip-to-city-lite>, or MaxMind's `GeoLite2-City.mmdb`
(free account and licence key, `geoipupdate`).

Set it once:

```sh
echo 'export GEOIP_DB="$HOME/.local/share/geohash-lookup/dbip-city-lite.mmdb"' >> ~/.zshrc
```

## 5. Name yourself to Nominatim

The public server throttles or refuses anonymous clients. The default User-Agent names this
project; name yours, with a contact:

```sh
echo 'export GEOHASH_USER_AGENT="my-tool/1.0 (me@example.org)"' >> ~/.zshrc
```

## 6. Self-hosted Nominatim (optional)

For volume, run your own instance and point the tool at it:

```sh
docker run -d --name nominatim -p 8080:8080 \
  -e PBF_URL=https://download.geofabrik.de/europe/albania-latest.osm.pbf \
  mediagis/nominatim:<version>  # community image: pin a version from its page

export NOMINATIM_URL=http://localhost:8080/search
export NOMINATIM_REVERSE_URL=http://localhost:8080/reverse
```

Regional extracts (Geofabrik) keep the disk small; all of France takes tens of GB. The 1.1 s
gap between two Nominatim requests applies to your instance too.

## 7. Releases (maintainers)

Consumers pin tags (`tag = "v0.1.1"`), never a branch.

1. On `main`, all checks pass:
   ```sh
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo clippy --no-default-features --all-targets -- -D warnings
   cargo test
   cargo deny check advisories licenses sources
   reuse lint
   ```
2. Try the real services once (not in the CI):
   `geohash-lookup --levels 2,4,5,6,7 Tokyo "Rue de Rivoli, Paris" "#u09tv"` and, with a
   fresh `--geoip-db` path, `8.8.8.8`.
3. Bump `version` in `Cargo.toml`, move `[Unreleased]` of `CHANGELOG.md` under the version
   and the date, commit `Release X.Y.Z`.
4. Tag and push: `git tag -a vX.Y.Z -m "geohash-lookup X.Y.Z" && git push origin main vX.Y.Z`.
5. Move the consumers' `tag = ` to the new tag, in their own repositories.

## 8. Troubleshooting

| Symptom | Likely cause / fix |
|---|---|
| `package requires rustc 1.88` | `rustup update stable` |
| `linker 'cc' not found` | install the C compiler (section 1) |
| `BAN request: ...` / `connect` | no outgoing HTTPS; set `HTTPS_PROXY` (honoured) |
| `Nominatim request: ... 403` or `429` | generic User-Agent or too many requests: `--user-agent`, space your calls |
| `no GeoIP file configured` | IP query without `--geoip-db` / `GEOIP_DB` |
| `GeoIP file: ...` when reading | not a **City** file (Country or ASN files do not work) |
| `DB-IP answer: ... not found` | DB-IP's file of this month and last month both missing: download it by hand |
| `not found` for an IP | private or reserved address, or missing from the file |
| `not found` for `#cell` | the centre of the cell is in the sea, or nowhere Nominatim names |
| a country gives a French street | use `--source nominatim`, or raise `--min-score` |
