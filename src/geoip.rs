// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GeoIP file: DB-IP's free "IP to City Lite", downloaded when missing.
//!
//! DB-IP publishes it monthly at `https://download.db-ip.com/free/dbip-city-lite-YYYY-MM.mmdb.gz`
//! under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/): show
//! [`DBIP_ATTRIBUTION`] with the results. About 30 MB to download, 130 MB on disk and in
//! memory.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::write::GzDecoder;
use maxminddb::Reader;

use crate::geocoder::{default_user_agent, http_client};
use crate::{Error, Result};

/// The credit DB-IP's licence asks for.
pub const DBIP_ATTRIBUTION: &str = "IP Geolocation by DB-IP (https://db-ip.com), CC BY 4.0";

/// Largest download accepted (the real one is about 30 MB).
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;
/// Connect and read timeout of the download (no limit on its total time).
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// URL of DB-IP's City Lite file for a month.
pub fn dbip_url(year: i32, month: u32) -> String {
    format!("https://download.db-ip.com/free/dbip-city-lite-{year:04}-{month:02}.mmdb.gz")
}

/// Downloads the GeoIP file to `path` if there is no file there. Returns the URL used, or
/// `None` if the file was already there. The file is checked (opened as an MMDB file) before
/// it takes its place; parent directories are created.
pub async fn ensure(path: &Path, user_agent: Option<&str>) -> Result<Option<String>> {
    if path.exists() {
        return Ok(None);
    }
    download(path, user_agent).await.map(Some)
}

/// Downloads this month's DB-IP City Lite file to `path` (last month's early in the month,
/// before DB-IP publishes), replacing any file there. Returns the URL used.
pub async fn download(path: &Path, user_agent: Option<&str>) -> Result<String> {
    let ua = user_agent.map_or_else(default_user_agent, str::to_owned);
    let client = http_client(&ua, DOWNLOAD_TIMEOUT, false)?;
    let (year, month) = year_month(SystemTime::now());
    let (py, pm) = if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    };
    let mut last = None;
    for url in [dbip_url(year, month), dbip_url(py, pm)] {
        match fetch_gunzip(&client, &url, path).await {
            Ok(()) => return Ok(url),
            // Not published yet: try last month's.
            Err(Error::Http { err, .. })
                if err.status() == Some(reqwest::StatusCode::NOT_FOUND) =>
            {
                last = Some(Error::BadResponse {
                    service: "DB-IP",
                    detail: format!("{url}: not found"),
                });
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.expect("two URLs tried"))
}

/// `GET url`, gunzipped into `path` through a temporary file next to it.
async fn fetch_gunzip(client: &reqwest::Client, url: &str, path: &Path) -> Result<()> {
    let http = |err| Error::Http {
        service: "DB-IP",
        err,
    };
    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(http)?
        .error_for_status()
        .map_err(http)?;

    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|err| io(dir, err))?;
    }
    let tmp = temporary(path);
    let result = async {
        let file = File::create(&tmp).map_err(|err| io(&tmp, err))?;
        let mut gz = GzDecoder::new(BufWriter::new(file));
        let mut total = 0u64;
        while let Some(chunk) = resp.chunk().await.map_err(http)? {
            total += chunk.len() as u64;
            if total > MAX_DOWNLOAD_BYTES {
                return Err(Error::BadResponse {
                    service: "DB-IP",
                    detail: format!("{url}: larger than {MAX_DOWNLOAD_BYTES} bytes"),
                });
            }
            gz.write_all(&chunk).map_err(|err| io(&tmp, err))?;
        }
        gz.finish()
            .and_then(|mut w| w.flush())
            .map_err(|err| io(&tmp, err))?;
        // A file that does not open is not kept.
        Reader::open_readfile(&tmp).map_err(|err| Error::GeoIpOpen {
            path: tmp.clone(),
            err,
        })?;
        std::fs::rename(&tmp, path).map_err(|err| io(path, err))
    }
    .await;
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn io(path: &Path, err: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        err,
    }
}

fn temporary(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".part");
    path.with_file_name(name)
}

/// UTC year and month of `t` (civil calendar from the day count, H. Hinnant's algorithm).
fn year_month(t: SystemTime) -> (i32, u32) {
    let days = t
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    (year, month)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn calendar() {
        assert_eq!(year_month(at(0)), (1970, 1));
        assert_eq!(year_month(at(951_782_400)), (2000, 2)); // 2000-02-29
        assert_eq!(year_month(at(1_793_318_400)), (2026, 10)); // 2026-10-30
        assert_eq!(year_month(at(1_798_761_599)), (2026, 12)); // 2026-12-31 23:59:59
        assert_eq!(year_month(at(1_798_761_600)), (2027, 1));
    }

    #[test]
    fn url_and_temporary_name() {
        assert_eq!(
            dbip_url(2026, 3),
            "https://download.db-ip.com/free/dbip-city-lite-2026-03.mmdb.gz"
        );
        assert_eq!(
            temporary(Path::new("/x/db.mmdb")),
            Path::new("/x/db.mmdb.part")
        );
    }

    #[tokio::test]
    async fn existing_file_is_not_downloaded() {
        let path = std::env::temp_dir().join(format!("geohash-lookup-{}.mmdb", std::process::id()));
        std::fs::write(&path, b"x").unwrap();
        assert_eq!(ensure(&path, None).await.unwrap(), None);
        std::fs::remove_file(&path).unwrap();
    }
}
