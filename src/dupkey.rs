//! Shared duplicate-key logic for `markdup` and `dedup`.
//!
//! Two profiles are considered duplicates when they share the same key, built
//! from `profile_timestamp` (formatted with a strftime string — the date only by
//! default) and `longitude`/`latitude` (rounded to a configurable number of
//! decimals). `platform_code` is deliberately **not** part of the key, so
//! duplicates are detected across platforms.

use std::error::Error;

use chrono::DateTime;

/// How a coordinate is reduced to its rounded integer key component.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum RoundMode {
    /// Round to nearest (ties away from zero)
    Round,
    /// Round toward negative infinity
    Floor,
    /// Round toward positive infinity
    Ceil,
    /// Round toward zero
    Trunc,
}

/// Options controlling how the duplicate key is derived. Defaults: date-only
/// timestamp (`%Y-%m-%d`), 3-decimal rounding, round-to-nearest.
#[derive(Clone, Debug)]
pub struct KeyOpts {
    pub time_format: String,
    pub decimals: u32,
    pub round_mode: RoundMode,
}

impl Default for KeyOpts {
    fn default() -> Self {
        Self { time_format: "%Y-%m-%d".to_string(), decimals: 3, round_mode: RoundMode::Round }
    }
}

/// A profile's duplicate key: (formatted timestamp, rounded longitude, rounded
/// latitude). The coordinates are scaled to integers so keys hash/compare exactly.
pub type DupKey = (String, i64, i64);

impl KeyOpts {
    /// Validate the strftime format string once, up front, so a bad format is a
    /// clean error rather than a panic when the first timestamp is formatted.
    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        use chrono::format::{Item, StrftimeItems};
        if StrftimeItems::new(&self.time_format).any(|i| matches!(i, Item::Error)) {
            return Err(format!("invalid --time-format '{}'", self.time_format).into());
        }
        Ok(())
    }

    /// Compute the key for a profile from its timestamp (Unix milliseconds, `None`
    /// if null) and longitude/latitude. Returns `None` when any component is
    /// missing (null timestamp or NaN position) — such a profile is never a
    /// duplicate.
    pub fn key(&self, ts_ms: Option<i64>, lon: f64, lat: f64) -> Option<DupKey> {
        let ts = ts_ms?;
        if lon.is_nan() || lat.is_nan() {
            return None;
        }
        let dt = DateTime::from_timestamp_millis(ts)?;
        let time = dt.naive_utc().format(&self.time_format).to_string();
        Some((time, self.round(lon), self.round(lat)))
    }

    /// Scale by `10^decimals`, apply the rounding mode, and return the integer.
    fn round(&self, v: f64) -> i64 {
        let x = v * 10f64.powi(self.decimals as i32);
        let r = match self.round_mode {
            RoundMode::Round => x.round(),
            RoundMode::Floor => x.floor(),
            RoundMode::Ceil => x.ceil(),
            RoundMode::Trunc => x.trunc(),
        };
        r as i64
    }
}
