# Usage

1. [Command line](#1-command-line)
2. [Rust library (API)](#2-rust-library-api)

---

## 1. Command line

```text
geohash-lookup [OPTIONS] <QUERIES>...
```

`<QUERIES>`: places, IP addresses, or `#geohash` to name a cell; one argument each (quoted
when it holds spaces).

### What a query is

| Typed | Taken as |
|---|---|
| `#u09t`, `#U09T`, `# u09t` | a geohash: reverse lookup, approximate name of the cell |
| `8.8.8.8`, `2001:4860:4860::8888`, `[::1]`, `fe80::1%en0` | an IP address (GeoIP file; zone after `%` ignored) |
| anything else, `u09t` included | a place to geocode |

A bare `u09t` is a place here: a valid geohash can also be a word ("bern", "def"). The
bitchat tools, whose fields took only geohashes so far, read a bare geohash as a geohash
(`CellInput::parse_lenient`): there `75001` is a cell (type `75001 Paris` for the place), and
one word mixing letters and digits that is not a geohash (`u0a`) is refused as a mistyped
geohash rather than looked up.

### Options

| Option | Environment | Default | Description |
|---|---|---|---|
| `--source <auto\|ip\|ban\|nominatim>` | | `auto` | Geocoding source (below) |
| `--precision <1..12>` | | from the result | Geohash length forced |
| `--min-precision <1..12>` | | `2` (*) | Cell of at least N characters (below) |
| `--max-precision <1..12>` | | `9` (*) | Cell of at most N characters (below) |
| `--levels <N,N...>` | | | Cell among these lengths, e.g. `2,4,5,6,7` (below) |
| `--geoip-db <FILE>` | `GEOIP_DB` | none | GeoIP City file (`.mmdb`), downloaded from DB-IP if missing |
| `--no-download` | | | Fail on a missing GeoIP file instead of downloading it |
| `--min-score <0..1>` | | `0.6` | Minimum BAN score with `auto` |
| `--ban-url <URL>` | `BAN_URL` | `https://data.geopf.fr/geocodage/search` | BAN search |
| `--ban-reverse-url <URL>` | `BAN_REVERSE_URL` | `https://data.geopf.fr/geocodage/reverse` | BAN reverse |
| `--nominatim-url <URL>` | `NOMINATIM_URL` | `https://nominatim.openstreetmap.org/search` | Nominatim search |
| `--nominatim-reverse-url <URL>` | `NOMINATIM_REVERSE_URL` | `https://nominatim.openstreetmap.org/reverse` | Nominatim reverse |
| `--language <LANG>` | | `en` | Language of Nominatim's names |
| `--user-agent <TEXT>` | `GEOHASH_USER_AGENT` | names this project | HTTP User-Agent: name yourself, with a contact |
| `--timeout <SECONDS>` | | `10` | Timeout of each HTTP request |
| `-h`, `--help` / `-V`, `--version` | | | Help, version |

(*) Defaults used only when one of the two is given. `--precision`, the bounds and `--levels`
exclude each other. A command-line option wins over its environment variable.

### Sources (`--source`)

| Value | Behaviour |
|---|---|
| `auto` | IP address: GeoIP file. Otherwise BAN (score >= `--min-score`; a street, an address or a locality only if the query names its town or postcode), then Nominatim when BAN fails, is unreachable or is not kept. |
| `ip` | GeoIP file only. Error if the query is not an IP address. |
| `ban` | BAN only, no score or town check; network errors are reported. |
| `nominatim` | Nominatim only. |

`#geohash` queries ignore `--source`: BAN reverse for cells of 7 characters or more (its
nearest French address, kept only if it lies inside the cell), Nominatim reverse otherwise or
when BAN has nothing there.

### Geohash length

Without options, the suggested length:

| Origin | Rule |
|---|---|
| BAN | house number 9, street 8, locality 7, town 5 |
| Nominatim | longest prefix shared by two opposite corners of the extent, 1 to 9 |
| GeoIP | accuracy radius of the file: <= 2 km 6, <= 10 km 5, <= 50 km 4, <= 200 km 3, more 2, none 5; 3 when the file knows only the country |

### Bounded cell (`--min-precision`, `--max-precision`, `--levels`)

The **finest cell that holds the whole extent of the place** (Nominatim's bounding box):

* an extent that would fit in a cell finer than the maximum (or the longest level): the
  maximum;
* an extent too large for a cell of the minimum (or the shortest level): the cell of that
  length **around the point** of the place. Tokyo's extent reaches the Ogasawara islands and
  fits only in `x`: with a minimum of 2, `xn`. Same for a country across the antimeridian
  (Fiji, -180 to 180);
* with `--levels`, the longest allowed length that still holds the extent (3 with levels
  2,4,5,6,7 gives 2);
* BAN and GeoIP give no extent: their suggested length (above), bounded the same way.

Cell sizes: 1 char ~5000 x 5000 km; 2 ~1250 x 625 km; 3 ~156 x 156 km; 4 ~39 x 19.5 km;
5 ~4.9 x 4.9 km; 6 ~1.2 x 0.6 km; 7 ~153 x 153 m; 8 ~38 x 19 m; 9 ~4.8 x 4.8 m.

### Reverse zoom

`#cell` asks Nominatim at a zoom matching the size of the cell: 1-2 characters a country, 3
a state, 4 a county, 5 a city, 6 a village or suburb, 7 major streets, 8 streets, 9+ a
building. The name is that of what lies at the centre of the cell: approximate, and missing
when the centre is in the sea. `lat`, `lon` are the centre of the cell.

### Output

Tab-separated, with a header line:

```text
query	geohash	lat	lon	source	label
```

| Column | Content |
|---|---|
| `query` | the query as typed (tabs and line breaks turned into spaces) |
| `geohash` | the cell |
| `lat`, `lon` | decimal degrees, WGS84, 6 decimals (the centre of the cell for `#cell`) |
| `source` | `GeoIP`, `BAN` or `Nominatim` |
| `label` | the service's name for it |

Nothing found: `<query>	-	-	-	-	not found`. Errors (network, missing file...) go to the
standard error as `<query>: <message>`.

### Exit codes

| Code | Meaning |
|---|---|
| `0` | every query found |
| `1` | a query not found or failed, or start-up impossible (unreadable GeoIP file...) |
| `2` | bad command-line arguments |

### Examples

```sh
# Places, at BitChat's channel levels
geohash-lookup --user-agent "me/1.0 (me@example.org)" --levels 2,4,5,6,7 \
  "Albania" "Guadeloupe" "Paris" "Rue de Rivoli, Paris"

# Name cells
geohash-lookup "#u09t" "#u09tvqx" "#xn"

# IP addresses (GeoIP file downloaded on the first run)
geohash-lookup --geoip-db ~/.local/share/geohash-lookup/dbip-city-lite.mmdb 8.8.8.8 2001:4860:4860::8888

# Finest cell holding the place, 2 to 6 characters (Tokyo -> xn)
geohash-lookup --min-precision 2 --max-precision 6 "Tokyo"

# Countries: Nominatim only (BAN covers France only)
geohash-lookup --source nominatim "Albania"

# Just the geohash
geohash-lookup "Paris" | tail -n +2 | cut -f2
```

---

## 2. Rust library (API)

A library (`geohash_lookup`) and a binary. The library is **async** (Tokio); `Geocoder` is
`Send + Sync`.

### Dependency

```toml
[dependencies]
geohash-lookup = { git = "https://github.com/wvmscs/geohash-lookup", tag = "v0.1.1", default-features = false }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

### A cell field

```rust
use geohash_lookup::{BITCHAT_LEVELS, CellInput, Geocoder, Source};

# async fn field(geocoder: &Geocoder, typed: &str) -> geohash_lookup::Result<()> {
match CellInput::parse_lenient(typed)? {
    CellInput::Geohash(g) => {
        // A cell: name it
        if let Some(place) = geocoder.reverse(&g).await? {
            println!("#{g} ≈ {} ({})", place.label, place.provider.attribution());
        }
    }
    CellInput::Query(q) => {
        // A place or an IP address: its cell at BitChat's levels
        if let Some(loc) = geocoder.locate(&q, Source::Auto).await? {
            let cell = loc.cell_at(&BITCHAT_LEVELS)?;
            println!("{q} -> #{} ({})", cell.geohash, loc.label);
        }
    }
}
# Ok(()) }
```

### Building

```rust
use geohash_lookup::{Geocoder, geoip};

# async fn build() -> Result<Geocoder, Box<dyn std::error::Error>> {
let db = std::path::Path::new("/var/lib/my-app/dbip-city-lite.mmdb");
geoip::ensure(db, Some("my-app/1.0 (contact@example.org)")).await?; // downloads if missing
let geocoder = Geocoder::builder()
    .user_agent("my-app/1.0 (contact@example.org)")
    .geoip_db(db)                                                    // optional: IP addresses
    .build()?;
# Ok(geocoder) }
```

Nominatim calls are serialised and at least 1.1 s apart, across tasks too. From synchronous
code: `tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(...)`.

### Reference

| Item | Description |
|---|---|
| `Geocoder::builder() -> GeocoderBuilder` | entry point |
| `GeocoderBuilder::user_agent`, `language`, `min_score`, `timeout` | settings |
| `GeocoderBuilder::ban_url`, `ban_reverse_url`, `nominatim_url`, `nominatim_reverse_url` | endpoints |
| `GeocoderBuilder::geoip_db(path)` | MMDB file, read into memory by `build()` |
| `GeocoderBuilder::build() -> Result<Geocoder>` | builds (installs ring as rustls provider if none) |
| `Geocoder::locate(&self, &str, Source) -> Result<Option<Location>>` | geocoding |
| `Geocoder::locate_ip(&self, IpAddr) -> Result<Option<Location>>` | GeoIP, **synchronous** |
| `Geocoder::reverse(&self, geohash) -> Result<Option<Location>>` | approximate name of a cell (`#` accepted) |
| `Geocoder::has_geoip(&self) -> bool` | a GeoIP file is loaded |
| `CellInput::parse(&str)` / `parse_lenient(&str)` | `Geohash(g)` or `Query(text)` |
| `is_geohash(&str)`, `parse_ip(&str)`, `encode_geohash(lat, lon, n)` | helpers |
| `Location { lat, lon, label, provider, precision, bbox }` | a result (`bbox`: Nominatim only) |
| `Location::geohash()` / `geohash_with_precision(n)` | suggested / given length |
| `Location::cell(min, max)` / `cell_at(&levels)` | finest cell holding the place, bounded |
| `Cell { geohash, precision, covers_place }` | `Some(true)` holds the whole extent; `Some(false)` forced to the minimum, holds the point only; `None` unknown extent |
| `BBox { south, north, west, east }` | extent |
| `Source` | `Auto` (default), `Ip`, `Ban`, `Nominatim` |
| `Provider` | `GeoIp`, `Ban`, `Nominatim`: `as_str()`, `attribution()`, `Display` |
| `geoip::ensure(path, ua)` / `geoip::download(path, ua)` / `geoip::dbip_url(y, m)` | DB-IP City Lite file |
| constants | `MAX_PRECISION`, `BITCHAT_LEVELS`, `DEFAULT_*` (endpoints, score, timeout, language, min and max precision), `geoip::DBIP_ATTRIBUTION` |

### What `locate` and `reverse` return

| Result | Meaning |
|---|---|
| `Ok(Some(loc))` | found |
| `Ok(None)` | nothing (IP missing from the file, centre of the cell in the sea...) |
| `Err(e)` | failure: network, missing or unreadable GeoIP file, `Source::Ip` with a non-IP, bad geohash... |

With `Source::Auto`, a BAN error is **not** returned (Nominatim is asked); call
`locate(.., Source::Ban)` to see it.

### Errors (`Error`)

| Variant | Cause |
|---|---|
| `Client` | HTTP client cannot be built |
| `Http { service, err }` | network or HTTP status of a service (`BAN`, `Nominatim`, `DB-IP`) |
| `BadResponse { service, detail }` | unusable answer |
| `GeoIpNotConfigured` | IP address without `geoip_db` |
| `GeoIpOpen { path, err }` | GeoIP file unreadable |
| `GeoIp(..)` | read or decode error (not a "City" file?) |
| `Io { path, err }` | download file cannot be written |
| `NotAnIp(..)` | `Source::Ip` with a non-IP query |
| `InvalidGeohash(..)` | `#` followed by something else than a geohash |
| `EmptyInput` | nothing typed |
| `InvalidPrecision(n)` | length outside `1..=12` |
| `InvalidBounds(..)` | `cell` with `min > max`, `cell_at` without levels |
| `Geohash(..)` | coordinates that cannot be encoded |

### Generated documentation

```sh
cargo doc --no-deps --no-default-features --open
```

### Known limits

* No cache: one query, one request (BAN, Nominatim). Add your own if needed.
* The first result only (`limit=1`).
* A small place across a cell border gets a coarse cell: the Eiffel Tower (about 125 m) fits
  only in a cell of 6 characters. A list of covering cells would do better.
* IP positions are **approximate**: VPNs, mobile networks and CDNs move them by hundreds of
  kilometres.
* With `auto`, "Rue de Rivoli" without its town goes to Nominatim: add the town or the
  postcode to stay on BAN.
