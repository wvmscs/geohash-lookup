---
name: geohash-lookup
description: Turn a place name, a postal address or an IP address into a geohash cell, or name a geohash cell approximately (reverse geocoding), with the geohash-lookup tool or Rust library. Use when asked which geohash covers a town, an address or a country, what place a geohash like u09t or #xn is, to pick a BitChat location channel for a place, or to wire geocoding into a Rust program.
---

# geohash-lookup

`geohash-lookup` turns a place or an IP address into a geohash cell, and a cell into an
approximate place name. Sources: BAN (French addresses), Nominatim (OpenStreetMap, the world),
a local GeoIP file (DB-IP City Lite) for IP addresses.

## Rules

- **The public Nominatim allows one request a second** and wants a User-Agent naming the
  caller. Pass `--user-agent "<tool>/<version> (<contact>)"` (or `GEOHASH_USER_AGENT`) and
  do not loop over hundreds of places against it: ask the user for a self-hosted instance
  (`NOMINATIM_URL`, `NOMINATIM_REVERSE_URL`) for volume.
- **Queries leave the machine**: BAN and Nominatim see the text and the IP address of the
  machine. Do not look up a private person's address, nor anything the user did not ask for.
  IP addresses are looked up locally, in the GeoIP file.
- **Credit the data** when showing results: "© OpenStreetMap contributors" (Nominatim),
  "Base Adresse Nationale" (BAN), "IP Geolocation by DB-IP" (GeoIP).
- **IP positions are approximate** (town at best; VPNs and mobile networks move them far).
  Never present one as where a person is.
- The GeoIP file is about 130 MB: download it (`--geoip-db` with a path that does not
  exist yet) only when an IP address is actually asked for, and ask before putting it
  outside the user's data directories.

## Command line

```sh
geohash-lookup [OPTIONS] <QUERIES>...      # one argument per query, quoted
```

| Typed | Gives |
|---|---|
| `"Rue de Rivoli, Paris"`, `"Tokyo"`, `"Albania"` | the cell of the place |
| `8.8.8.8`, `2001:4860:4860::8888` | the cell of the IP address (needs `--geoip-db`) |
| `"#u09t"` (with `#`) | the approximate name of the cell |

A bare `u09t` is a place name, not a cell: add `#`.

Output: tab-separated, header `query geohash lat lon source label`; `not found` lines; errors
on stderr; exit 1 if anything failed.

Choose the cell length:

| Need | Option |
|---|---|
| a BitChat location channel (region 2, province 4, city 5, neighbourhood 6, block 7) | `--levels 2,4,5,6,7` |
| the finest cell holding the whole place, bounded | `--min-precision 2 --max-precision 6` |
| a fixed length | `--precision 5` |
| countries or places outside France | `--source nominatim` (skips BAN) |

`--levels` and the bounds give the finest cell that holds the **whole extent** of the place;
when nothing allowed holds it (Tokyo reaches the Ogasawara islands, Fiji crosses the
antimeridian), the shortest allowed cell **around the point** of the place: Tokyo -> `xn`.

```sh
geohash-lookup --levels 2,4,5,6,7 "Paris" "Tokyo"           # u09tv, xn
geohash-lookup "#u09tv" "#xn"                               # Paris..., Japan
geohash-lookup --geoip-db ~/.local/share/geohash-lookup/dbip-city-lite.mmdb 8.8.8.8
geohash-lookup "Paris" | tail -n +2 | cut -f2               # the geohash only
```

Reading results:

- `source` BAN with a street or an address label: precise (5 to 9 characters).
- `source` Nominatim: the length follows the place's extent (a country: 1 to 3 characters).
- A `#cell` name is what lies at the **centre** of the cell; `not found` when the centre is
  in the sea. A 4-character cell is named by its county, a 5-character one by its town.
- "Rue de Rivoli" alone goes to Nominatim (any Rue de Rivoli): add the town or postcode.

## Library (Rust)

```toml
geohash-lookup = { git = "https://github.com/wvmscs/geohash-lookup", tag = "v0.1.0", default-features = false }
```

```rust
use geohash_lookup::{BITCHAT_LEVELS, CellInput, Geocoder, Source};

let geocoder = Geocoder::builder().user_agent("my-app/1.0 (me@example.org)").build()?;
match CellInput::parse_lenient(typed)? {           // "#u09t", "u09t", "Paris", "8.8.8.8"
    CellInput::Geohash(g) => { let name = geocoder.reverse(&g).await?; }
    CellInput::Query(q) => {
        if let Some(loc) = geocoder.locate(&q, Source::Auto).await? {
            let cell = loc.cell_at(&BITCHAT_LEVELS)?; // cell.geohash, cell.covers_place
        }
    }
}
```

`Geocoder` is `Send + Sync` (share one in an `Arc`); Nominatim calls are spaced 1.1 s apart
across tasks. `geohash_lookup::geoip::ensure(path, ua)` downloads the GeoIP file if missing;
then `.geoip_db(path)` on the builder. Full reference: `USAGE.md` of the repository.

## In the bitchat tools

bitchat-console (top "geohash" field, `bitchat-console <cell or place>`) and bitchat-geohash
(the "area" field, `bitchat-geohash <cell or place>`), built with the `geocode` feature, take
`#u09t`, a bare geohash, a place or an IP address (with `--geoip-db`), and show the
approximate name of the cell followed.
