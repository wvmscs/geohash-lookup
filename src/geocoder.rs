// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: GPL-3.0-or-later

//! The geocoder: BAN, Nominatim and the GeoIP file, both ways.

use std::net::IpAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use maxminddb::{Reader, geoip2};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::{
    BBox, DEFAULT_BAN_REVERSE_URL, DEFAULT_BAN_URL, DEFAULT_LANGUAGE, DEFAULT_MIN_SCORE,
    DEFAULT_NOMINATIM_REVERSE_URL, DEFAULT_NOMINATIM_URL, DEFAULT_TIMEOUT, Error, Location,
    Provider, Result, Source, bbox_level, is_geohash, parse_ip,
};

/// Minimum gap between two Nominatim requests.
const NOMINATIM_MIN_GAP: Duration = Duration::from_millis(1100);

/// Shortest cell named by BAN's reverse search (about 150 m): coarser cells get a town or a
/// region from Nominatim rather than one address.
const BAN_REVERSE_MIN_PRECISION: usize = 7;

/// Installs ring as the process-wide rustls crypto provider, if none is.
pub(crate) fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// The HTTP client of the library. `total`: a timeout for the whole request (`None` for
/// downloads, which only get connect and read timeouts).
pub(crate) fn http_client(
    user_agent: &str,
    timeout: Duration,
    total: bool,
) -> Result<reqwest::Client> {
    install_crypto_provider();
    let b = reqwest::Client::builder()
        .user_agent(user_agent)
        .connect_timeout(timeout)
        .read_timeout(timeout);
    let b = if total { b.timeout(timeout) } else { b };
    b.build().map_err(Error::Client)
}

/// Default User-Agent: names the crate; set your own, with a contact.
pub(crate) fn default_user_agent() -> String {
    format!(
        "geohash-lookup/{} (+https://github.com/wvmscs/geohash-lookup)",
        env!("CARGO_PKG_VERSION")
    )
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builds a [`Geocoder`].
#[derive(Clone, Debug)]
pub struct GeocoderBuilder {
    user_agent: String,
    ban_url: String,
    ban_reverse_url: String,
    nominatim_url: String,
    nominatim_reverse_url: String,
    language: String,
    min_score: f64,
    timeout: Duration,
    geoip_db: Option<PathBuf>,
}

impl Default for GeocoderBuilder {
    fn default() -> Self {
        Self {
            user_agent: default_user_agent(),
            ban_url: DEFAULT_BAN_URL.to_owned(),
            ban_reverse_url: DEFAULT_BAN_REVERSE_URL.to_owned(),
            nominatim_url: DEFAULT_NOMINATIM_URL.to_owned(),
            nominatim_reverse_url: DEFAULT_NOMINATIM_REVERSE_URL.to_owned(),
            language: DEFAULT_LANGUAGE.to_owned(),
            min_score: DEFAULT_MIN_SCORE,
            timeout: DEFAULT_TIMEOUT,
            geoip_db: None,
        }
    }
}

impl GeocoderBuilder {
    /// User-Agent of the HTTP requests. The public Nominatim asks for a real name of your
    /// application with a way to reach you.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    /// BAN search endpoint (default [`DEFAULT_BAN_URL`]).
    pub fn ban_url(mut self, url: impl Into<String>) -> Self {
        self.ban_url = url.into();
        self
    }

    /// BAN reverse endpoint (default [`DEFAULT_BAN_REVERSE_URL`]).
    pub fn ban_reverse_url(mut self, url: impl Into<String>) -> Self {
        self.ban_reverse_url = url.into();
        self
    }

    /// Nominatim search endpoint (default [`DEFAULT_NOMINATIM_URL`]).
    pub fn nominatim_url(mut self, url: impl Into<String>) -> Self {
        self.nominatim_url = url.into();
        self
    }

    /// Nominatim reverse endpoint (default [`DEFAULT_NOMINATIM_REVERSE_URL`]).
    pub fn nominatim_reverse_url(mut self, url: impl Into<String>) -> Self {
        self.nominatim_reverse_url = url.into();
        self
    }

    /// Language of Nominatim's names (`Accept-Language`, default [`DEFAULT_LANGUAGE`]).
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }

    /// Minimum BAN score (0..1) with [`Source::Auto`].
    pub fn min_score(mut self, score: f64) -> Self {
        self.min_score = score;
        self
    }

    /// Timeout of each HTTP request.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// A GeoIP "City" file in MMDB format (needed for IP addresses), read into memory by
    /// [`GeocoderBuilder::build`]. [`crate::geoip::ensure`] downloads it if missing.
    pub fn geoip_db(mut self, path: impl Into<PathBuf>) -> Self {
        self.geoip_db = Some(path.into());
        self
    }

    /// Builds the [`Geocoder`] (opens the GeoIP file if any). Installs ring as the
    /// process-wide rustls crypto provider if none is.
    pub fn build(self) -> Result<Geocoder> {
        let client = http_client(&self.user_agent, self.timeout, true)?;
        let geoip = match self.geoip_db {
            Some(path) => {
                Some(Reader::open_readfile(&path).map_err(|err| Error::GeoIpOpen { path, err })?)
            }
            None => None,
        };
        Ok(Geocoder {
            client,
            ban_url: self.ban_url,
            ban_reverse_url: self.ban_reverse_url,
            nominatim_url: self.nominatim_url,
            nominatim_reverse_url: self.nominatim_reverse_url,
            language: self.language,
            min_score: self.min_score,
            geoip,
            last_nominatim: Mutex::new(None),
        })
    }
}

// ---------------------------------------------------------------------------
// Answers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct BanResponse {
    features: Vec<BanFeature>,
}

#[derive(Deserialize)]
struct BanFeature {
    geometry: BanGeometry,
    properties: BanProps,
}

#[derive(Deserialize)]
struct BanGeometry {
    /// GeoJSON: [lon, lat]
    coordinates: [f64; 2],
}

#[derive(Deserialize)]
pub(crate) struct BanProps {
    pub(crate) label: String,
    pub(crate) score: f64,
    /// housenumber | street | locality | municipality
    #[serde(rename = "type")]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) city: Option<String>,
    #[serde(default)]
    pub(crate) postcode: Option<String>,
}

#[derive(Deserialize)]
struct NominatimItem {
    lat: String,
    lon: String,
    display_name: String,
    /// [south, north, west, east] as strings
    boundingbox: Vec<String>,
}

/// `/reverse` answers an object, or `{"error": "Unable to geocode"}`.
#[derive(Deserialize)]
struct NominatimReverse {
    #[serde(default)]
    display_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Geocoder
// ---------------------------------------------------------------------------

/// Place or IP -> [`Location`] -> geohash, and geohash -> approximate place.
///
/// Built with [`Geocoder::builder`]. `Send + Sync`, to share between tasks.
pub struct Geocoder {
    client: reqwest::Client,
    ban_url: String,
    ban_reverse_url: String,
    nominatim_url: String,
    nominatim_reverse_url: String,
    language: String,
    min_score: f64,
    geoip: Option<Reader<Vec<u8>>>,
    last_nominatim: Mutex<Option<Instant>>,
}

impl Geocoder {
    /// Starts building a geocoder.
    pub fn builder() -> GeocoderBuilder {
        GeocoderBuilder::default()
    }

    /// True if a GeoIP file is loaded.
    pub fn has_geoip(&self) -> bool {
        self.geoip.is_some()
    }

    /// Geocodes `query` with `source`.
    ///
    /// * `Ok(Some(_))`: found;
    /// * `Ok(None)`: asked, nothing found (or BAN not kept with `Auto` and Nominatim found
    ///   nothing, or an IP address missing from the file);
    /// * `Err(_)`: network error, no GeoIP file, bad query...
    pub async fn locate(&self, query: &str, source: Source) -> Result<Option<Location>> {
        match source {
            Source::Ip => {
                let ip = parse_ip(query).ok_or_else(|| Error::NotAnIp(query.to_owned()))?;
                self.locate_ip(ip)
            }
            Source::Ban => self.locate_ban(query, 0.0, false).await,
            Source::Nominatim => self.locate_nominatim(query).await,
            Source::Auto => {
                if let Some(ip) = parse_ip(query) {
                    return self.locate_ip(ip);
                }
                // A BAN error (network, HTTP) does not stop the fallback on Nominatim.
                if let Ok(Some(loc)) = self.locate_ban(query, self.min_score, true).await {
                    return Ok(Some(loc));
                }
                self.locate_nominatim(query).await
            }
        }
    }

    /// Locates an IP address (v4 or v6) in the local GeoIP file.
    ///
    /// Approximate (town, region), never an address. Private or reserved addresses are not
    /// in public files: `Ok(None)`.
    pub fn locate_ip(&self, ip: IpAddr) -> Result<Option<Location>> {
        let reader = self.geoip.as_ref().ok_or(Error::GeoIpNotConfigured)?;
        let result = reader.lookup(ip)?;
        let Some(city) = result.decode::<geoip2::City>()? else {
            return Ok(None);
        };
        let (Some(lat), Some(lon)) = (city.location.latitude, city.location.longitude) else {
            return Ok(None);
        };

        let town = city.city.names.english;
        let mut parts: Vec<&str> = Vec::new();
        if let Some(c) = town {
            parts.push(c);
        }
        if let Some(c) = city.country.names.english.or(city.country.iso_code) {
            parts.push(c);
        }
        let label = if parts.is_empty() {
            ip.to_string()
        } else {
            format!("{ip} ({})", parts.join(", "))
        };
        // Without a town, the point is the centre of a country (or a region).
        let precision = if town.is_some() {
            ip_precision(city.location.accuracy_radius)
        } else {
            3
        };
        Ok(Some(Location {
            lat,
            lon,
            label,
            provider: Provider::GeoIp,
            precision,
            bbox: None,
        }))
    }

    /// Names a cell approximately: BAN's nearest address for a cell of 7 characters or more
    /// inside France, otherwise Nominatim at a zoom matching the size of the cell (country,
    /// region, town, district, street). The location returned is the centre of the cell.
    ///
    /// `Ok(None)`: nothing there (open sea...).
    pub async fn reverse(&self, geohash: &str) -> Result<Option<Location>> {
        let g = geohash.trim().trim_start_matches('#').to_ascii_lowercase();
        if !is_geohash(&g) {
            return Err(Error::InvalidGeohash(geohash.to_owned()));
        }
        let rect = geohash::decode_bbox(&g).map_err(|e| Error::Geohash(e.to_string()))?;
        let (lo, hi) = (rect.min(), rect.max());
        let (lat, lon) = ((lo.y + hi.y) / 2.0, (lo.x + hi.x) / 2.0);
        let cell = BBox {
            south: lo.y,
            north: hi.y,
            west: lo.x,
            east: hi.x,
        };
        let found = |label: String, provider| Location {
            lat,
            lon,
            label,
            provider,
            precision: g.len(),
            bbox: None,
        };

        // BAN answers its nearest French address, however far: kept only inside the cell.
        if g.len() >= BAN_REVERSE_MIN_PRECISION
            && let Ok(Some((label, (plat, plon)))) = self.reverse_ban(lat, lon).await
            && (cell.south..=cell.north).contains(&plat)
            && (cell.west..=cell.east).contains(&plon)
        {
            return Ok(Some(found(label, Provider::Ban)));
        }
        Ok(self
            .reverse_nominatim(lat, lon, nominatim_zoom(g.len()))
            .await?
            .map(|label| found(label, Provider::Nominatim)))
    }

    async fn locate_ban(
        &self,
        q: &str,
        min_score: f64,
        require_context: bool,
    ) -> Result<Option<Location>> {
        let http = |err| Error::Http {
            service: "BAN",
            err,
        };
        let resp: BanResponse = self
            .client
            .get(&self.ban_url)
            .query(&[("q", q), ("limit", "1")])
            .send()
            .await
            .map_err(http)?
            .error_for_status()
            .map_err(http)?
            .json()
            .await
            .map_err(http)?;

        let Some(f) = resp.features.into_iter().next() else {
            return Ok(None);
        };
        if f.properties.score < min_score {
            return Ok(None);
        }
        // Without the town in the query, "Albanie" or "Tokyo" give "Rue Albanie Regourd,
        // Toulouse" or "Place de Tokyo, Paris", scored about 0.7.
        if require_context
            && f.properties.kind != "municipality"
            && !query_mentions_place(q, &f.properties)
        {
            return Ok(None);
        }
        let precision = match f.properties.kind.as_str() {
            "housenumber" => 9,
            "street" => 8,
            "locality" => 7,
            // A town: its city cell (about 4.9 km), BAN gives no extent.
            "municipality" => 5,
            _ => 7,
        };
        Ok(Some(Location {
            lat: f.geometry.coordinates[1],
            lon: f.geometry.coordinates[0],
            label: f.properties.label,
            provider: Provider::Ban,
            precision,
            bbox: None,
        }))
    }

    /// BAN's nearest address: its label and its point.
    async fn reverse_ban(&self, lat: f64, lon: f64) -> Result<Option<(String, (f64, f64))>> {
        let http = |err| Error::Http {
            service: "BAN",
            err,
        };
        let resp: BanResponse = self
            .client
            .get(&self.ban_reverse_url)
            .query(&[
                ("lat", lat.to_string()),
                ("lon", lon.to_string()),
                ("limit", "1".into()),
            ])
            .send()
            .await
            .map_err(http)?
            .error_for_status()
            .map_err(http)?
            .json()
            .await
            .map_err(http)?;
        Ok(resp.features.into_iter().next().map(|f| {
            let [plon, plat] = f.geometry.coordinates;
            (f.properties.label, (plat, plon))
        }))
    }

    /// Public Nominatim's policy: one request a second at most. The lock is held while
    /// waiting, which also orders concurrent tasks.
    async fn nominatim_turn(&self) {
        let mut last = self.last_nominatim.lock().await;
        if let Some(t) = *last {
            let elapsed = t.elapsed();
            if elapsed < NOMINATIM_MIN_GAP {
                tokio::time::sleep(NOMINATIM_MIN_GAP - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    async fn locate_nominatim(&self, q: &str) -> Result<Option<Location>> {
        self.nominatim_turn().await;
        let http = |err| Error::Http {
            service: "Nominatim",
            err,
        };
        let items: Vec<NominatimItem> = self
            .client
            .get(&self.nominatim_url)
            .query(&[
                ("q", q),
                ("format", "jsonv2"),
                ("limit", "1"),
                ("accept-language", self.language.as_str()),
            ])
            .send()
            .await
            .map_err(http)?
            .error_for_status()
            .map_err(http)?
            .json()
            .await
            .map_err(http)?;

        let Some(it) = items.into_iter().next() else {
            return Ok(None);
        };
        let bad = |detail: String| Error::BadResponse {
            service: "Nominatim",
            detail,
        };
        let lat: f64 = it.lat.parse().map_err(|e| bad(format!("lat: {e}")))?;
        let lon: f64 = it.lon.parse().map_err(|e| bad(format!("lon: {e}")))?;

        let bbox = match it.boundingbox.as_slice() {
            [s, n, w, e] => match (s.parse(), n.parse(), w.parse(), e.parse()) {
                (Ok(south), Ok(north), Ok(west), Ok(east)) => Some(BBox {
                    south,
                    north,
                    west,
                    east,
                }),
                _ => None,
            },
            _ => None,
        };
        let precision = bbox.as_ref().map_or(6, |b| bbox_level(b).clamp(1, 9));
        Ok(Some(Location {
            lat,
            lon,
            label: it.display_name,
            provider: Provider::Nominatim,
            precision,
            bbox,
        }))
    }

    async fn reverse_nominatim(&self, lat: f64, lon: f64, zoom: u8) -> Result<Option<String>> {
        self.nominatim_turn().await;
        let http = |err| Error::Http {
            service: "Nominatim",
            err,
        };
        let r: NominatimReverse = self
            .client
            .get(&self.nominatim_reverse_url)
            .query(&[
                ("lat", lat.to_string()),
                ("lon", lon.to_string()),
                ("zoom", zoom.to_string()),
                ("format", "jsonv2".into()),
                ("accept-language", self.language.clone()),
            ])
            .send()
            .await
            .map_err(http)?
            .error_for_status()
            .map_err(http)?
            .json()
            .await
            .map_err(http)?;
        Ok(r.display_name)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Nominatim's zoom for a cell (3 country, 5 state, 8 county, 10 city, 13 village or
/// suburb, 16 major streets, 17 streets, 18 building), by its size: 2 characters ~1250 km,
/// 3 ~156 km, 4 ~39 km, 5 ~4.9 km, 6 ~1.2 km, 7 ~153 m.
fn nominatim_zoom(precision: usize) -> u8 {
    match precision {
        0..=2 => 3,
        3 => 5,
        4 => 8,
        5 => 10,
        6 => 13,
        7 => 16,
        8 => 17,
        _ => 18,
    }
}

/// Geohash precision from the accuracy radius of the GeoIP file (km, 67 % confidence).
fn ip_precision(radius_km: Option<u16>) -> usize {
    match radius_km {
        None => 5,
        Some(0..=2) => 6,    // cell ~1.2 km
        Some(3..=10) => 5,   // ~4.9 km
        Some(11..=50) => 4,  // ~39 km
        Some(51..=200) => 3, // ~156 km
        Some(_) => 2,
    }
}

/// True if the query holds the town or the postcode of the BAN result (whole words, ignoring
/// case, French accents, hyphens and apostrophes).
pub(crate) fn query_mentions_place(query: &str, props: &BanProps) -> bool {
    let q = format!(" {} ", normalize(query));
    [&props.city, &props.postcode]
        .into_iter()
        .flatten()
        .map(|s| normalize(s))
        .any(|s| !s.is_empty() && q.contains(&format!(" {s} ")))
}

/// Lower case, French accents removed, anything not alphanumeric -> space, spaces merged.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'â' | 'ä' => out.push('a'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'î' | 'ï' => out.push('i'),
            'ô' | 'ö' => out.push('o'),
            'ù' | 'û' | 'ü' => out.push('u'),
            'ÿ' => out.push('y'),
            'ç' => out.push('c'),
            'œ' => out.push_str("oe"),
            'æ' => out.push_str("ae"),
            c if c.is_alphanumeric() => out.push(c),
            _ => out.push(' '),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ban_props(kind: &str, city: &str, postcode: &str) -> BanProps {
        BanProps {
            label: String::new(),
            score: 0.7,
            kind: kind.into(),
            city: Some(city.into()),
            postcode: Some(postcode.into()),
        }
    }

    #[test]
    fn normalisation() {
        assert_eq!(normalize("Saint-Étienne"), "saint etienne");
        assert_eq!(normalize("  L'Haÿ-les-Roses "), "l hay les roses");
        assert_eq!(normalize("Œuilly, 02160"), "oeuilly 02160");
    }

    #[test]
    fn ban_context() {
        let toulouse = ban_props("street", "Toulouse", "31000");
        assert!(!query_mentions_place("Albanie", &toulouse));
        assert!(query_mentions_place(
            "rue albanie regourd toulouse",
            &toulouse
        ));
        assert!(query_mentions_place("Rue Albanie Regourd 31000", &toulouse));

        let caen = ban_props("housenumber", "Caen", "14000");
        assert!(query_mentions_place(
            "1 esplanade Jean-Marie Louvel, Caen",
            &caen
        ));
        // whole words: "Paris" must not accept "Parisot"
        let parisot = ban_props("street", "Parisot", "81310");
        assert!(!query_mentions_place("Rue de la Gare, Paris", &parisot));
    }

    #[test]
    fn ip_precision_from_radius() {
        assert_eq!(ip_precision(None), 5);
        assert_eq!(ip_precision(Some(1)), 6);
        assert_eq!(ip_precision(Some(20)), 4);
        assert_eq!(ip_precision(Some(1000)), 2);
    }

    #[test]
    fn zoom_follows_cell_size() {
        assert_eq!(nominatim_zoom(1), 3);
        assert_eq!(nominatim_zoom(4), 8);
        assert_eq!(nominatim_zoom(5), 10);
        assert_eq!(nominatim_zoom(7), 16);
        assert_eq!(nominatim_zoom(12), 18);
    }

    #[tokio::test]
    async fn ip_without_file_is_a_clear_error() {
        let g = Geocoder::builder().build().unwrap();
        let err = g.locate("8.8.8.8", Source::Auto).await.unwrap_err();
        assert!(matches!(err, Error::GeoIpNotConfigured));
        let err = g.locate("Paris", Source::Ip).await.unwrap_err();
        assert!(matches!(err, Error::NotAnIp(_)));
        let err = g.reverse("#paris").await.unwrap_err();
        assert!(matches!(err, Error::InvalidGeohash(_)));
    }
}
