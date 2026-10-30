// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command line of the `geohash_lookup` library.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use geohash_lookup::{
    CellInput, DEFAULT_BAN_REVERSE_URL, DEFAULT_BAN_URL, DEFAULT_LANGUAGE, DEFAULT_MAX_PRECISION,
    DEFAULT_MIN_PRECISION, DEFAULT_MIN_SCORE, DEFAULT_NOMINATIM_REVERSE_URL, DEFAULT_NOMINATIM_URL,
    Geocoder, Location, Source, geoip,
};

#[derive(Parser)]
#[command(
    version,
    about = "Place or IP address -> geohash (local GeoIP, BAN, Nominatim), #geohash -> place",
    after_help = "Data: © OpenStreetMap contributors (ODbL); Base Adresse Nationale (Licence \
                  Ouverte); IP Geolocation by DB-IP (https://db-ip.com), CC BY 4.0."
)]
struct Cli {
    /// Places or IPv4/IPv6 addresses to geocode, or `#geohash` to name a cell (one argument
    /// each, quoted)
    #[arg(required = true)]
    queries: Vec<String>,

    /// Geocoding source
    #[arg(long, value_enum, default_value_t = CliSource::Auto)]
    source: CliSource,

    /// Geohash length forced (1..=12). Otherwise from the result.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=12))]
    precision: Option<u8>,

    /// Finest cell holding the whole place, at least N characters (otherwise the cell of N
    /// characters around the point). Default with --max-precision: 2.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=12), conflicts_with = "precision")]
    min_precision: Option<u8>,

    /// Finest cell holding the whole place, at most N characters. Default with
    /// --min-precision: 9 (or --min-precision if larger).
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=12), conflicts_with = "precision")]
    max_precision: Option<u8>,

    /// Finest cell holding the whole place among these lengths, e.g. 2,4,5,6,7 (BitChat's
    /// channels); otherwise the shortest, around the point.
    #[arg(long, value_delimiter = ',', value_parser = clap::value_parser!(u8).range(1..=12),
          conflicts_with_all = ["precision", "min_precision", "max_precision"])]
    levels: Vec<u8>,

    /// GeoIP City file in MMDB format, needed for IP addresses; downloaded from DB-IP if
    /// missing
    #[arg(long, env = "GEOIP_DB")]
    geoip_db: Option<PathBuf>,

    /// Do not download a missing GeoIP file: fail instead
    #[arg(long)]
    no_download: bool,

    /// Minimum BAN score (0..1) to accept a result with --source auto
    #[arg(long, default_value_t = DEFAULT_MIN_SCORE)]
    min_score: f64,

    /// BAN search endpoint (IGN Géoplateforme by default)
    #[arg(long, env = "BAN_URL", default_value = DEFAULT_BAN_URL)]
    ban_url: String,

    /// BAN reverse endpoint
    #[arg(long, env = "BAN_REVERSE_URL", default_value = DEFAULT_BAN_REVERSE_URL)]
    ban_reverse_url: String,

    /// Nominatim search endpoint (your own instance here)
    #[arg(long, env = "NOMINATIM_URL", default_value = DEFAULT_NOMINATIM_URL)]
    nominatim_url: String,

    /// Nominatim reverse endpoint
    #[arg(long, env = "NOMINATIM_REVERSE_URL", default_value = DEFAULT_NOMINATIM_REVERSE_URL)]
    nominatim_reverse_url: String,

    /// Language of Nominatim's names
    #[arg(long, default_value = DEFAULT_LANGUAGE)]
    language: String,

    /// User-Agent naming you (asked by the public Nominatim's usage policy)
    #[arg(long, env = "GEOHASH_USER_AGENT")]
    user_agent: Option<String>,

    /// Timeout of each HTTP request, in seconds
    #[arg(long, default_value_t = 10)]
    timeout: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliSource {
    /// IP -> GeoIP file; otherwise BAN first, Nominatim next
    Auto,
    /// GeoIP file only (the query must be an IP address)
    Ip,
    /// BAN only
    Ban,
    /// Nominatim only
    Nominatim,
}

impl From<CliSource> for Source {
    fn from(s: CliSource) -> Self {
        match s {
            CliSource::Auto => Source::Auto,
            CliSource::Ip => Source::Ip,
            CliSource::Ban => Source::Ban,
            CliSource::Nominatim => Source::Nominatim,
        }
    }
}

/// How the geohash of a found place is chosen.
enum Choice {
    Suggested,
    Fixed(usize),
    Bounds(usize, usize),
    Levels(Vec<usize>),
}

impl Choice {
    fn geohash(&self, loc: &Location) -> geohash_lookup::Result<String> {
        match self {
            Choice::Suggested => loc.geohash(),
            Choice::Fixed(p) => loc.geohash_with_precision(*p),
            Choice::Bounds(min, max) => loc.cell(*min, *max).map(|c| c.geohash),
            Choice::Levels(l) => loc.cell_at(l).map(|c| c.geohash),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut builder = Geocoder::builder()
        .ban_url(cli.ban_url)
        .ban_reverse_url(cli.ban_reverse_url)
        .nominatim_url(cli.nominatim_url)
        .nominatim_reverse_url(cli.nominatim_reverse_url)
        .language(cli.language)
        .min_score(cli.min_score)
        .timeout(Duration::from_secs(cli.timeout));
    if let Some(ua) = &cli.user_agent {
        builder = builder.user_agent(ua);
    }
    if let Some(path) = cli.geoip_db {
        if !cli.no_download && !path.exists() {
            eprintln!(
                "downloading the DB-IP City Lite file to {}...",
                path.display()
            );
            let url = geoip::ensure(&path, cli.user_agent.as_deref()).await?;
            if let Some(url) = url {
                eprintln!("downloaded {url} ({})", geoip::DBIP_ATTRIBUTION);
            }
        }
        builder = builder.geoip_db(path);
    }
    let geocoder = builder.build()?;
    let source: Source = cli.source.into();
    let choice = match (cli.precision, cli.min_precision, cli.max_precision) {
        _ if !cli.levels.is_empty() => {
            Choice::Levels(cli.levels.iter().map(|&l| usize::from(l)).collect())
        }
        (Some(p), _, _) => Choice::Fixed(usize::from(p)),
        (None, None, None) => Choice::Suggested,
        (None, min, max) => {
            let min = min.map_or(DEFAULT_MIN_PRECISION, usize::from);
            let max = max.map_or(DEFAULT_MAX_PRECISION.max(min), usize::from);
            Choice::Bounds(min, max)
        }
    };

    println!("query\tgeohash\tlat\tlon\tsource\tlabel");
    let mut failed = false;

    for q in &cli.queries {
        // Tabs and line breaks would break the TSV.
        let shown = q.replace(['\t', '\n', '\r'], " ");
        let found = match CellInput::parse(q) {
            Err(e) => Err(e),
            Ok(CellInput::Geohash(g)) => geocoder
                .reverse(&g)
                .await
                .map(|r| r.map(|loc| (g.clone(), loc))),
            Ok(CellInput::Query(text)) => match geocoder.locate(&text, source).await {
                Ok(Some(loc)) => choice.geohash(&loc).map(|g| Some((g, loc))),
                Ok(None) => Ok(None),
                Err(e) => Err(e),
            },
        };
        match found {
            Ok(Some((gh, loc))) => {
                let label = loc.label.replace(['\t', '\n', '\r'], " ");
                println!(
                    "{shown}\t{gh}\t{:.6}\t{:.6}\t{}\t{label}",
                    loc.lat, loc.lon, loc.provider
                );
            }
            Ok(None) => {
                failed = true;
                println!("{shown}\t-\t-\t-\t-\tnot found");
            }
            Err(e) => {
                failed = true;
                eprintln!("{shown}: {e}");
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
    Ok(())
}
