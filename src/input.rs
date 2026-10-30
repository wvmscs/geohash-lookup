// SPDX-FileCopyrightText: 2026 William VITAL, FR (wvmscs)
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a user types in a cell field: a geohash (`#u09t`) or a text to geocode.

use crate::{Error, MAX_PRECISION, Result};

/// The geohash alphabet (no `a`, `i`, `l`, `o`).
const ALPHABET: &str = "0123456789bcdefghjkmnpqrstuvwxyz";

/// True if `s` is a geohash: 1 to 12 characters of the alphabet, any case.
pub fn is_geohash(s: &str) -> bool {
    (1..=MAX_PRECISION).contains(&s.len())
        && s.chars().all(|c| ALPHABET.contains(c.to_ascii_lowercase()))
}

/// A cell field's content, sorted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellInput {
    /// A geohash, lower case.
    Geohash(String),
    /// A place or an IP address, trimmed.
    Query(String),
}

impl CellInput {
    /// `#` then a geohash is a geohash (`#U09T` -> `u09t`); anything else is a query, even
    /// `u09t` (a valid geohash can be a word: "bern", "def").
    pub fn parse(typed: &str) -> Result<CellInput> {
        let s = typed.trim();
        if let Some(g) = s.strip_prefix('#') {
            let g = g.trim();
            if !is_geohash(g) {
                return Err(Error::InvalidGeohash(g.to_owned()));
            }
            return Ok(CellInput::Geohash(g.to_ascii_lowercase()));
        }
        if s.is_empty() {
            return Err(Error::EmptyInput);
        }
        Ok(CellInput::Query(s.to_owned()))
    }

    /// Like [`CellInput::parse`], a bare geohash being a geohash too: for fields and command
    /// lines that took only geohashes so far. "Bern" is then the cell `bern`, "75001" the cell
    /// `75001`; type "Bern, Switzerland" or "75001 Paris" for the places.
    ///
    /// One word of 12 characters at most mixing letters and digits (`u0a`) is a mistyped
    /// geohash: [`Error::InvalidGeohash`], not a place to look up.
    pub fn parse_lenient(typed: &str) -> Result<CellInput> {
        let s = typed.trim();
        if is_geohash(s) {
            return Ok(CellInput::Geohash(s.to_ascii_lowercase()));
        }
        let mixed = s.len() <= MAX_PRECISION
            && s.chars().all(|c| c.is_ascii_alphanumeric())
            && s.chars().any(|c| c.is_ascii_digit())
            && s.chars().any(|c| c.is_ascii_alphabetic());
        if mixed {
            return Err(Error::InvalidGeohash(s.to_owned()));
        }
        CellInput::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geohash_alphabet() {
        assert!(is_geohash("u09t"));
        assert!(is_geohash("U09TVQ"));
        assert!(!is_geohash(""));
        assert!(!is_geohash("paris")); // a, i
        assert!(!is_geohash("u09tvqxxxxxxx")); // 13
        assert!(!is_geohash("u0 9t"));
    }

    #[test]
    fn strict_parse() {
        let g = |s: &str| CellInput::Geohash(s.into());
        let q = |s: &str| CellInput::Query(s.into());
        assert_eq!(CellInput::parse(" #U09T ").unwrap(), g("u09t"));
        assert_eq!(CellInput::parse("# u09t").unwrap(), g("u09t"));
        assert_eq!(CellInput::parse("u09t").unwrap(), q("u09t"));
        assert_eq!(CellInput::parse(" Paris ").unwrap(), q("Paris"));
        assert_eq!(CellInput::parse("8.8.8.8").unwrap(), q("8.8.8.8"));
        assert!(matches!(
            CellInput::parse("#paris"),
            Err(Error::InvalidGeohash(_))
        ));
        assert!(matches!(
            CellInput::parse("#"),
            Err(Error::InvalidGeohash(_))
        ));
        assert!(matches!(CellInput::parse("   "), Err(Error::EmptyInput)));
    }

    #[test]
    fn lenient_parse() {
        let g = |s: &str| CellInput::Geohash(s.into());
        assert_eq!(CellInput::parse_lenient("U09TV").unwrap(), g("u09tv"));
        assert_eq!(CellInput::parse_lenient("#u09tv").unwrap(), g("u09tv"));
        assert_eq!(CellInput::parse_lenient("75001").unwrap(), g("75001"));
        // A mistyped geohash is not a place
        assert!(matches!(
            CellInput::parse_lenient("u0a"),
            Err(Error::InvalidGeohash(_))
        ));
        assert_eq!(
            CellInput::parse_lenient("75001 Paris").unwrap(),
            CellInput::Query("75001 Paris".into())
        );
        assert_eq!(
            CellInput::parse_lenient("Albania").unwrap(),
            CellInput::Query("Albania".into())
        );
        assert_eq!(
            CellInput::parse_lenient("Paris").unwrap(),
            CellInput::Query("Paris".into())
        );
    }
}
