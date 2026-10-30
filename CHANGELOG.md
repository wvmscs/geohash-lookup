# Changelog

## [Unreleased]

## [0.1.0] - 2026-10-30

First version, public.

### Added
- `Geocoder::locate`: place or IP address to a location, through BAN, Nominatim or a local
  GeoIP file (`Source::Auto`: IP -> GeoIP, otherwise BAN then Nominatim). A BAN street,
  address or locality is kept only if the query names its town or postcode.
- `Location::cell(min, max)` and `Location::cell_at(&levels)`: the finest cell holding the
  whole extent of the place, bounded; around the point when the extent is too large or
  crosses the antimeridian (Tokyo `xn`, Fiji `ru`). `BITCHAT_LEVELS`.
- `Geocoder::reverse`: approximate name of a cell (Nominatim at a zoom matching the cell,
  BAN for 7+ characters in France).
- `CellInput`: `#geohash` or a text; `parse_lenient` also takes a bare geohash.
- `geoip::ensure` / `download`: DB-IP's monthly "IP to City Lite" file, downloaded when
  missing and checked before use.
- `Provider::attribution`: the data credits.
- Command line `geohash-lookup`: `#cell` queries, `--levels`, `--min-precision`,
  `--max-precision`, `--geoip-db` with download, `--no-download`, `--language`, reverse
  endpoints.
- GPL-3.0-or-later, REUSE, CI (fmt, clippy, tests, cargo deny, reuse lint).
