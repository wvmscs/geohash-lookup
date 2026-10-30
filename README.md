# geohash-lookup

A **place** (address, town, country...) or an **IP address** (IPv4/IPv6) to a
[geohash](https://en.wikipedia.org/wiki/Geohash) cell, and a **cell back to an approximate
place name**. A command-line tool **and** an async Rust library, meant for the cell fields of
[bitchat-console](https://github.com/wvmscs/bitchat-console) and
[bitchat-geohash](https://github.com/wvmscs/bitchat-geohash): type `#u09t`, or `Paris`.

```text
$ geohash-lookup --levels 2,4,5,6,7 "Tokyo" "Rue de Rivoli, Paris" "#u09tv" 8.8.8.8
query                 geohash  lat        lon         source     label
Tokyo                 xn       35.676860  139.763895  Nominatim  Tokyo, Japan
Rue de Rivoli, Paris  u09tvpj  48.862442  2.336200    BAN        Rue de Rivoli 75001 Paris
#u09tv                u09tv    48.845215  2.351074    Nominatim  Paris, Ile-de-France, ..., France
8.8.8.8               9q9hv    37.422000  -122.085000 GeoIP      8.8.8.8 (Mountain View, United States)
```

The output is tab-separated, one line per query (aligned here). See [USAGE.md](USAGE.md).

## What it does

| Typed | Source | Result |
|---|---|---|
| `#` and a geohash | Nominatim reverse (BAN reverse for 7+ characters in France) | approximate name of the cell |
| IPv4 / IPv6 address | local GeoIP file (`.mmdb`), downloaded from DB-IP if missing | town-level cell |
| French address | BAN (IGN Géoplateforme) | 5 (town) to 9 (house number) characters |
| anything else (country, region, world) | Nominatim (OpenStreetMap) | from the extent of the place |

The cell is **the finest one that holds the whole place**, within bounds (`--min-precision`,
`--max-precision`) or among allowed lengths (`--levels`, BitChat's 2, 4, 5, 6, 7). When the
place is too large or awkward for any allowed cell (Tokyo's extent reaches the Ogasawara
islands, Fiji crosses the antimeridian), the shortest allowed cell **around the point** of the
place: Tokyo gives `xn` rather than `x`.

With `--source auto` (the default), BAN is asked first, then Nominatim. A street, an address
or a locality from BAN is kept only if the query names its town or postcode, so that
"Albanie" or "Tokyo" go to Nominatim instead of "Rue Albanie, Toulouse" or "Place de Tokyo,
Paris".

## Files

| File | Role |
|---|---|
| [INSTALL.md](INSTALL.md) | installation, GeoIP file, self-hosted Nominatim, releases |
| [USAGE.md](USAGE.md) | command line, output, exit codes, Rust API |
| [skills/geohash-lookup/SKILL.md](skills/geohash-lookup/SKILL.md) | skill for coding agents |
| [AGENTS.md](AGENTS.md), [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md) | for contributors |
| [CHANGELOG.md](CHANGELOG.md) | versions |

## Quick start

```sh
cargo install --git https://github.com/wvmscs/geohash-lookup --tag v0.1.1 --locked
geohash-lookup --user-agent "me/1.0 (me@example.org)" "Paris" "#u09t"
```

## Privacy

Queries go **straight** to BAN and Nominatim (no Tor, no proxy beyond the system's): those
services see your IP address and what you look up. IP addresses are looked up locally, in the
GeoIP file; only its download reaches DB-IP.

## Data licences

Show the credits next to the results (`Provider::attribution` in the library):

* **Nominatim / OpenStreetMap**: ODbL data, "© OpenStreetMap contributors". The public
  server allows 1 request a second and asks for a User-Agent naming you
  ([usage policy](https://operations.osmfoundation.org/policies/nominatim/)).
* **BAN**: Licence Ouverte (Etalab).
* **DB-IP Lite**: [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), "IP Geolocation
  by DB-IP" with a link to <https://db-ip.com>. GeoLite2 files (MaxMind, account needed) work
  too.

## Licence

GPL-3.0-or-later. Every file says so with its SPDX tags ([REUSE](https://reuse.software)): a
header, or `REUSE.toml`; the licence text is in `LICENSES/`.
