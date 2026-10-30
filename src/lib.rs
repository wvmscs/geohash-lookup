// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: GPL-3.0-or-later

//! # geohash-lookup
//!
//! A place (free text) or an IP address (v4/v6) to a [geohash](https://en.wikipedia.org/wiki/Geohash)
//! cell, and a cell back to an approximate place name.
//!
//! Three sources, chosen automatically with [`Source::Auto`]:
//!
//! 1. **GeoIP**: an IPv4 or IPv6 address is looked up in a local MMDB file (DB-IP Lite City,
//!    GeoLite2-City...). Approximate position. [`geoip::ensure`] downloads DB-IP's file.
//! 2. **BAN** (Base Adresse Nationale, French IGN Géoplateforme): French addresses, with a
//!    minimum score ([`GeocoderBuilder::min_score`]). A street, an address or a locality is
//!    kept only if the query names its town or postcode (otherwise "Albanie" would give
//!    "Rue Albanie, Toulouse").
//! 3. **Nominatim** (OpenStreetMap): everything else (countries, regions, the world).
//!
//! [`Location::cell`] and [`Location::cell_at`] give the finest cell that holds the whole
//! extent of the place, within bounds or allowed levels; when the extent is too large (Tokyo
//! and the Ogasawara islands, Fiji across the antimeridian), the cell of the coarsest allowed
//! level that holds the point of the place (Tokyo: `xn` rather than `x`).
//! [`Geocoder::reverse`] names a cell approximately. [`CellInput`] tells a typed geohash
//! (`#u09t`) from a text to geocode.
//!
//! ## Example
//!
//! ```no_run
//! use geohash_lookup::{CellInput, Geocoder, Source};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let geocoder = Geocoder::builder()
//!     .user_agent("my-app/1.0 (contact@example.org)") // required by Nominatim
//!     .build()?;
//!
//! for typed in ["#u09t", "Tokyo", "Rue de Rivoli, Paris"] {
//!     match CellInput::parse(typed)? {
//!         CellInput::Geohash(g) => {
//!             let place = geocoder.reverse(&g).await?;
//!             println!("{g} ≈ {}", place.map_or("?".into(), |p| p.label));
//!         }
//!         CellInput::Query(q) => {
//!             if let Some(loc) = geocoder.locate(&q, Source::Auto).await? {
//!                 let cell = loc.cell_at(&[2, 4, 5, 6, 7])?;
//!                 println!("{q} -> {} ({})", cell.geohash, loc.provider);
//!             }
//!         }
//!     }
//! }
//! # Ok(()) }
//! ```
//!
//! ## Usage notes
//!
//! * A [`Geocoder`] is `Send + Sync`: build one and share it (`Arc`).
//! * Nominatim requests are at least 1.1 s apart (policy of the public server), across
//!   concurrent tasks too. Point [`GeocoderBuilder::nominatim_url`] to your own instance for
//!   volume.
//! * With [`Source::Auto`], a BAN error (network, HTTP) is not returned: Nominatim is asked
//!   instead. Use [`Source::Ban`] to see the error.
//! * Requests go straight to the services (no proxy beyond the system's): they see the
//!   caller's IP address and the query.

use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use geohash::Coord;
use maxminddb::MaxMindDbError;

mod geocoder;
pub mod geoip;
mod input;

pub use geocoder::{Geocoder, GeocoderBuilder};
pub use input::{CellInput, is_geohash};

/// Longest geohash accepted.
pub const MAX_PRECISION: usize = 12;
/// Default BAN search endpoint (IGN Géoplateforme).
pub const DEFAULT_BAN_URL: &str = "https://data.geopf.fr/geocodage/search";
/// Default BAN reverse endpoint.
pub const DEFAULT_BAN_REVERSE_URL: &str = "https://data.geopf.fr/geocodage/reverse";
/// Default Nominatim search endpoint (public OpenStreetMap server).
pub const DEFAULT_NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org/search";
/// Default Nominatim reverse endpoint.
pub const DEFAULT_NOMINATIM_REVERSE_URL: &str = "https://nominatim.openstreetmap.org/reverse";
/// Default minimum BAN score with [`Source::Auto`].
pub const DEFAULT_MIN_SCORE: f64 = 0.6;
/// Default HTTP timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
/// Default language of Nominatim's names.
pub const DEFAULT_LANGUAGE: &str = "en";
/// Default minimum of [`Location::cell`] (Tokyo: `xn` rather than `x`).
pub const DEFAULT_MIN_PRECISION: usize = 2;
/// Default maximum of [`Location::cell`] (about 4.8 x 4.8 m).
pub const DEFAULT_MAX_PRECISION: usize = 9;
/// The levels of BitChat's location channels: region, province, city, neighbourhood, block.
pub const BITCHAT_LEVELS: [usize; 5] = [2, 4, 5, 6, 7];

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors of the library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The HTTP client cannot be built.
    #[error("HTTP client: {0}")]
    Client(#[source] reqwest::Error),

    /// Network or HTTP error from a service.
    #[error("{service} request: {err}")]
    Http {
        service: &'static str,
        #[source]
        err: reqwest::Error,
    },

    /// Unusable answer from a service.
    #[error("{service} answer: {detail}")]
    BadResponse {
        service: &'static str,
        detail: String,
    },

    /// An IP address was asked for but no GeoIP file is configured.
    #[error(
        "no GeoIP file configured (command line: --geoip-db or GEOIP_DB; library: \
         GeocoderBuilder::geoip_db); a City file in MMDB format (DB-IP Lite or GeoLite2)"
    )]
    GeoIpNotConfigured,

    /// The GeoIP file cannot be opened.
    #[error("GeoIP file {}: {err}", path.display())]
    GeoIpOpen {
        path: PathBuf,
        #[source]
        err: MaxMindDbError,
    },

    /// Read or decode error in the GeoIP file (is it a "City" file?).
    #[error("GeoIP file: {0}")]
    GeoIp(#[from] MaxMindDbError),

    /// A file of the GeoIP download cannot be written.
    #[error("{}: {err}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        err: std::io::Error,
    },

    /// `Source::Ip` with a query that is not an IP address.
    #[error("{0:?} is not a valid IPv4/IPv6 address")]
    NotAnIp(String),

    /// Not a geohash (alphabet `0-9 b-h j k m n p-z`, 1 to 12 characters).
    #[error("{0:?} is not a geohash")]
    InvalidGeohash(String),

    /// Nothing typed.
    #[error("nothing to look up")]
    EmptyInput,

    /// Precision outside `1..=MAX_PRECISION`.
    #[error("invalid geohash precision: {0} (expected 1..={MAX_PRECISION})")]
    InvalidPrecision(usize),

    /// Bounds of [`Location::cell`] in the wrong order (`min > max`), or no level for
    /// [`Location::cell_at`].
    #[error("invalid precision bounds: {0}")]
    InvalidBounds(String),

    /// Coordinates that cannot be encoded.
    #[error("geohash encoding: {0}")]
    Geohash(String),
}

/// Result of the library.
pub type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Source asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// IP -> GeoIP; otherwise BAN, then Nominatim.
    #[default]
    Auto,
    /// The GeoIP file only (the query must be an IP address).
    Ip,
    /// BAN only (no minimum score, no town check).
    Ban,
    /// Nominatim only.
    Nominatim,
}

/// Service that answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    GeoIp,
    Ban,
    Nominatim,
}

impl Provider {
    /// Short name: `"GeoIP"`, `"BAN"` or `"Nominatim"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::GeoIp => "GeoIP",
            Provider::Ban => "BAN",
            Provider::Nominatim => "Nominatim",
        }
    }

    /// The credit the data licence asks for, to show next to a result.
    pub fn attribution(&self) -> &'static str {
        match self {
            Provider::GeoIp => geoip::DBIP_ATTRIBUTION,
            Provider::Ban => "Base Adresse Nationale (Licence Ouverte, Etalab)",
            Provider::Nominatim => "© OpenStreetMap contributors (ODbL)",
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A geocoding result.
#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    /// Latitude (decimal degrees, WGS84).
    pub lat: f64,
    /// Longitude (decimal degrees, WGS84).
    pub lon: f64,
    /// Name given by the service (normalised address, full name...).
    pub label: String,
    /// Service that answered.
    pub provider: Provider,
    /// Suggested geohash precision (1..=12), from the kind of result.
    pub precision: usize,
    /// Extent of the place, when the service gives one (Nominatim only).
    pub bbox: Option<BBox>,
}

/// Extent of a place, in decimal degrees (WGS84).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BBox {
    pub south: f64,
    pub north: f64,
    pub west: f64,
    pub east: f64,
}

/// A geohash cell from [`Location::cell`] or [`Location::cell_at`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Geohash of the cell.
    pub geohash: String,
    /// Its length.
    pub precision: usize,
    /// `Some(true)`: the cell holds the whole extent of the place; `Some(false)`: the level
    /// was forced up and the cell holds only the point of the place; `None`: unknown extent
    /// (BAN, GeoIP), level estimated from the kind of result.
    pub covers_place: Option<bool>,
}

impl Location {
    /// Geohash at the suggested precision ([`Location::precision`]).
    pub fn geohash(&self) -> Result<String> {
        encode_geohash(self.lat, self.lon, self.precision)
    }

    /// Geohash at a given precision (1..=12).
    pub fn geohash_with_precision(&self, precision: usize) -> Result<String> {
        encode_geohash(self.lat, self.lon, precision)
    }

    /// The finest cell that holds the whole extent of the place, `min..=max` characters long.
    ///
    /// When the extent does not fit in a cell of `min` characters, the cell of `min`
    /// characters that holds the point of the place. Without a known extent, the level starts
    /// from [`Location::precision`].
    pub fn cell(&self, min: usize, max: usize) -> Result<Cell> {
        for p in [min, max] {
            check_precision(p)?;
        }
        if min > max {
            return Err(Error::InvalidBounds(format!("min {min} > max {max}")));
        }
        self.cell_of(self.natural_level().clamp(min, max))
    }

    /// Like [`Location::cell`], with only some levels allowed (BitChat's channels:
    /// [`BITCHAT_LEVELS`]): the finest allowed level that holds the whole extent, or else the
    /// coarsest allowed level, around the point of the place.
    pub fn cell_at(&self, levels: &[usize]) -> Result<Cell> {
        let mut levels = levels.to_vec();
        levels.sort_unstable();
        levels.dedup();
        let Some(&coarsest) = levels.first() else {
            return Err(Error::InvalidBounds("no level allowed".into()));
        };
        for &p in &levels {
            check_precision(p)?;
        }
        let natural = self.natural_level();
        let level = levels
            .iter()
            .rev()
            .find(|&&l| l <= natural)
            .copied()
            .unwrap_or(coarsest);
        self.cell_of(level)
    }

    /// Finest level that holds the extent (0: none), or the suggested precision.
    fn natural_level(&self) -> usize {
        self.bbox.as_ref().map_or(self.precision, bbox_level)
    }

    fn cell_of(&self, precision: usize) -> Result<Cell> {
        let geohash = encode_geohash(self.lat, self.lon, precision)?;
        let covers_place = match &self.bbox {
            Some(b) => Some(cell_contains(&geohash, b)?),
            None => None,
        };
        Ok(Cell {
            geohash,
            precision,
            covers_place,
        })
    }
}

fn check_precision(p: usize) -> Result<()> {
    if (1..=MAX_PRECISION).contains(&p) {
        Ok(())
    } else {
        Err(Error::InvalidPrecision(p))
    }
}

/// Encodes coordinates as a geohash (`precision`: 1..=12 characters).
pub fn encode_geohash(lat: f64, lon: f64, precision: usize) -> Result<String> {
    check_precision(precision)?;
    geohash::encode(Coord { x: lon, y: lat }, precision).map_err(|e| Error::Geohash(e.to_string()))
}

/// Recognises an IPv4 or IPv6 address, `[::1]` and `fe80::1%en0` too (zone ignored).
pub fn parse_ip(query: &str) -> Option<IpAddr> {
    let s = query.trim().trim_start_matches('[').trim_end_matches(']');
    let s = s.split('%').next().unwrap_or(s);
    s.parse().ok()
}

/// Largest precision (0..=12) such that the whole bbox fits in one cell; 0 if it fits in
/// none. A cell being a rectangle (convex), two opposite corners sharing a prefix suffice.
fn bbox_level(b: &BBox) -> usize {
    // Across the antimeridian: no cell holds it.
    if b.west > b.east || b.south > b.north {
        return 0;
    }
    let sw = geohash::encode(
        Coord {
            x: b.west,
            y: b.south,
        },
        MAX_PRECISION,
    );
    let ne = geohash::encode(
        Coord {
            x: b.east,
            y: b.north,
        },
        MAX_PRECISION,
    );
    match (sw, ne) {
        (Ok(a), Ok(b)) => a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count(),
        _ => 0,
    }
}

/// True if the cell `geohash` holds the whole bbox.
fn cell_contains(geohash: &str, b: &BBox) -> Result<bool> {
    let r = geohash::decode_bbox(geohash).map_err(|e| Error::Geohash(e.to_string()))?;
    let (lo, hi) = (r.min(), r.max());
    Ok(b.west <= b.east
        && b.south <= b.north
        && lo.x <= b.west
        && b.east <= hi.x
        && lo.y <= b.south
        && b.north <= hi.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn nominatim(lat: f64, lon: f64, bbox: [f64; 4]) -> Location {
        let [south, north, west, east] = bbox;
        let b = BBox {
            south,
            north,
            west,
            east,
        };
        Location {
            lat,
            lon,
            label: "test".into(),
            provider: Provider::Nominatim,
            precision: bbox_level(&b).clamp(1, 9),
            bbox: Some(b),
        }
    }

    fn ban(precision: usize) -> Location {
        Location {
            lat: 57.64911,
            lon: 10.40744,
            label: "test".into(),
            provider: Provider::Ban,
            precision,
            bbox: None,
        }
    }

    #[test]
    fn known_geohash() {
        // Classic example: (57.64911, 10.40744) -> u4pruydqqvj
        assert_eq!(
            encode_geohash(57.64911, 10.40744, 11).unwrap(),
            "u4pruydqqvj"
        );
    }

    #[test]
    fn invalid_precision() {
        assert!(matches!(
            encode_geohash(0.0, 0.0, 0),
            Err(Error::InvalidPrecision(0))
        ));
        assert!(matches!(
            encode_geohash(0.0, 0.0, 13),
            Err(Error::InvalidPrecision(13))
        ));
    }

    #[test]
    fn location_geohash() {
        let loc = ban(5);
        assert_eq!(loc.geohash().unwrap(), "u4pru");
        assert_eq!(loc.geohash_with_precision(11).unwrap(), "u4pruydqqvj");
    }

    #[test]
    fn ip_detection() {
        assert!(parse_ip("8.8.8.8").is_some());
        assert!(parse_ip(" 2001:4860:4860::8888 ").is_some());
        assert!(parse_ip("[::1]").is_some());
        assert!(parse_ip("fe80::1%en0").is_some());
        assert!(parse_ip("Paris").is_none());
        assert!(parse_ip("10 rue de la Paix").is_none());
        assert!(parse_ip("999.1.1.1").is_none());
    }

    #[test]
    fn tiny_bbox_gives_high_precision() {
        // A bbox of about 50 cm, by the Eiffel Tower: fits in a cell of 9 characters (~5 m)
        let p = nominatim(
            48.8582025,
            2.2945025,
            [48.8582, 48.858205, 2.2945, 2.294505],
        );
        assert_eq!(p.precision, 9);
    }

    #[test]
    fn large_bbox_gives_low_precision() {
        // Albania, roughly
        let p = nominatim(41.18, 19.88, [39.6, 42.7, 19.2, 21.1]).precision;
        assert!(p <= 3, "precision {p}");
    }

    #[test]
    fn cell_forced_to_the_minimum() {
        // Tokyo: the bbox holds the Ogasawara islands and fits only in "x"
        let tokyo = nominatim(35.67686, 139.763895, [20.2, 35.9, 136.0, 154.0]);
        let c = tokyo.cell(2, 9).unwrap();
        assert_eq!(c.geohash, "xn");
        assert_eq!(c.covers_place, Some(false));
        let c = tokyo.cell(1, 9).unwrap();
        assert_eq!(c.geohash, "x");
        assert_eq!(c.covers_place, Some(true));

        // Fiji: -180..180, no cell holds the bbox
        let fiji = nominatim(-18.12397, 179.012274, [-21.94, -12.26, -180.0, 180.0]);
        let c = fiji.cell(2, 9).unwrap();
        assert_eq!(c.precision, 2);
        assert_eq!(c.covers_place, Some(false));
    }

    #[test]
    fn cell_capped_to_the_maximum() {
        let p = nominatim(
            48.8582025,
            2.2945025,
            [48.8582, 48.858205, 2.2945, 2.294505],
        );
        let c = p.cell(2, 6).unwrap();
        assert_eq!(c.precision, 6);
        assert_eq!(c.covers_place, Some(true));
    }

    #[test]
    fn cell_without_extent_and_bad_bounds() {
        let loc = ban(9);
        let c = loc.cell(2, 7).unwrap();
        assert_eq!((c.geohash.as_str(), c.covers_place), ("u4pruyd", None));
        assert!(matches!(loc.cell(0, 7), Err(Error::InvalidPrecision(0))));
        assert!(matches!(loc.cell(8, 7), Err(Error::InvalidBounds(_))));
        assert!(matches!(loc.cell_at(&[]), Err(Error::InvalidBounds(_))));
        assert!(matches!(
            loc.cell_at(&[2, 13]),
            Err(Error::InvalidPrecision(13))
        ));
    }

    #[test]
    fn cell_at_bitchat_levels() {
        // Tokyo (natural level 1) -> forced to 2
        let tokyo = nominatim(35.67686, 139.763895, [20.2, 35.9, 136.0, 154.0]);
        let c = tokyo.cell_at(&BITCHAT_LEVELS).unwrap();
        assert_eq!((c.geohash.as_str(), c.covers_place), ("xn", Some(false)));
        // Natural level 3 -> 2, which still holds the place
        let p = nominatim(41.18, 19.88, [41.0, 41.3, 19.7, 20.0]);
        assert_eq!(bbox_level(p.bbox.as_ref().unwrap()), 3);
        let c = p.cell_at(&BITCHAT_LEVELS).unwrap();
        assert_eq!((c.precision, c.covers_place), (2, Some(true)));
        // An address (9) -> block level, 7
        assert_eq!(ban(9).cell_at(&BITCHAT_LEVELS).unwrap().geohash, "u4pruyd");
        // Order and repeats do not matter
        assert_eq!(ban(9).cell_at(&[7, 2, 7, 5]).unwrap().precision, 7);
    }

    #[test]
    fn geocoder_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Geocoder>();
    }
}
