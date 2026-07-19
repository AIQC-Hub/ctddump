//! Compare two Parquet files and report how far each one covers the other.
//!
//! Profiles are matched on a key built the same way as the duplicate key (see
//! [`crate::dupkey`]): the profile time reduced to a date, plus longitude and
//! latitude rounded to a fixed number of decimals. Unlike the duplicate key,
//! `platform_code` **is** part of the key by default, since two files being
//! compared are usually different extracts of the same platforms; pass
//! `--no-platform-key` to match on time and position alone.
//!
//! Coverage is reported in both directions, because it is not symmetric: a file
//! can be fully contained in a larger one (100% covered) while covering only a
//! fraction of it. The second file is the reference in the first row, the first
//! file in the second.
//!
//! Memory is bounded the same way as `markdup`: each file is streamed in
//! `chunk_rows()` slices and reduced to one record per profile, so the peak is
//! proportional to the profile count, not the observation count.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;

use polars::prelude::*;

use crate::cli::ReportFormat;
use crate::convert::common;
use crate::dupkey::KeyOpts;
use crate::report::format::write_report;

/// Unix milliseconds at 1950-01-01, the oceanographic epoch that `profile_time`
/// counts days from.
const EPOCH_1950_MS: i64 = -631_152_000_000;

/// A profile's comparison key: the optional platform code followed by the
/// formatted date and the scaled longitude/latitude.
type CmpKey = (Option<String>, String, i64, i64);

/// Which columns to read and whether the platform code takes part in the key.
#[derive(Clone, Debug)]
pub struct CompareOpts {
    pub key: KeyOpts,
    pub platform_col: String,
    pub time_col: String,
    pub lon_col: String,
    pub lat_col: String,
    /// When false the key is time and position only, so the same cast matched
    /// under two different platform codes still counts as common.
    pub use_platform: bool,
}

impl Default for CompareOpts {
    fn default() -> Self {
        Self {
            key: KeyOpts::default(),
            platform_col: "platform_code".to_string(),
            time_col: "profile_time".to_string(),
            lon_col: "longitude".to_string(),
            lat_col: "latitude".to_string(),
            use_platform: true,
        }
    }
}

/// How the time column carries its value. `profile_time` is days since 1950,
/// `profile_timestamp` is a datetime, and either may be named as the time key.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TimeKind {
    Days1950,
    Datetime,
}

/// Days since 1950-01-01 to Unix milliseconds. Non-finite input has no date, so
/// it yields `None` rather than silently landing on the epoch.
fn days1950_to_ms(days: f64) -> Option<i64> {
    if !days.is_finite() {
        return None;
    }
    Some((days * 86_400_000.0).round() as i64 + EPOCH_1950_MS)
}

/// One file reduced to per-profile records.
struct Scanned {
    /// `(key, n_obs)` per profile; the key is `None` when the profile has no
    /// usable time or position and so can never match.
    profiles: Vec<(Option<CmpKey>, u64)>,
    platforms: HashSet<String>,
    observations: u64,
}

/// The other file indexed for lookup: key to the set of observation counts of
/// the profiles carrying it (a set, because several profiles may share a key).
type KeyIndex = HashMap<CmpKey, HashSet<u64>>;

impl Scanned {
    fn index(&self) -> KeyIndex {
        let mut idx: KeyIndex = HashMap::new();
        for (key, n_obs) in &self.profiles {
            if let Some(k) = key {
                idx.entry(k.clone()).or_default().insert(*n_obs);
            }
        }
        idx
    }
}

pub fn run(
    opts: &CompareOpts,
    format: ReportFormat,
    a: &Path,
    b: &Path,
    dest: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    opts.key.validate()?;

    let scanned_a = scan_file(opts, a)?;
    let scanned_b = scan_file(opts, b)?;
    let index_a = scanned_a.index();
    let index_b = scanned_b.index();

    let label_a = label(a);
    let label_b = label(b);

    // The second file is the reference first, as the more common question is how
    // much of the newer/larger file the older one already covers.
    let rows = vec![
        direction(&label_b, &label_a, &scanned_b, &index_a, &scanned_a.platforms),
        direction(&label_a, &label_b, &scanned_a, &index_b, &scanned_b.platforms),
    ];

    let df = build_df(&rows)?;
    write_report(&df, format, dest)?;
    Ok(())
}

/// One direction's statistics.
struct Row {
    reference: String,
    compared: String,
    ref_platforms: u32,
    common_platforms: u32,
    platform_cov_pct: f64,
    ref_profiles: u32,
    ref_unkeyed_profiles: u32,
    matched_profiles: u32,
    profile_cov_pct: f64,
    same_nobs: u32,
    diff_nobs: u32,
    nobs_agree_pct: f64,
    ref_observations: u64,
    matched_observations: u64,
}

/// Percentage, rounded to two decimals. An empty denominator has no meaningful
/// percentage, so it becomes NaN, which every output format renders as empty.
fn pct(num: u64, den: u64) -> f64 {
    if den == 0 {
        return f64::NAN;
    }
    ((num as f64 / den as f64) * 10_000.0).round() / 100.0
}

fn direction(
    reference: &str,
    compared: &str,
    r: &Scanned,
    other: &KeyIndex,
    other_platforms: &HashSet<String>,
) -> Row {
    let common_platforms = r.platforms.iter().filter(|p| other_platforms.contains(*p)).count();

    let mut unkeyed = 0u32;
    let mut matched = 0u32;
    let mut same_nobs = 0u32;
    let mut matched_obs = 0u64;
    for (key, n_obs) in &r.profiles {
        let Some(k) = key else {
            unkeyed += 1;
            continue;
        };
        if let Some(counts) = other.get(k) {
            matched += 1;
            matched_obs += n_obs;
            // A key shared by several profiles counts as agreeing when any of
            // them has the same observation count.
            if counts.contains(n_obs) {
                same_nobs += 1;
            }
        }
    }

    Row {
        reference: reference.to_string(),
        compared: compared.to_string(),
        ref_platforms: r.platforms.len() as u32,
        common_platforms: common_platforms as u32,
        platform_cov_pct: pct(common_platforms as u64, r.platforms.len() as u64),
        ref_profiles: r.profiles.len() as u32,
        ref_unkeyed_profiles: unkeyed,
        matched_profiles: matched,
        profile_cov_pct: pct(matched as u64, r.profiles.len() as u64),
        same_nobs,
        diff_nobs: matched - same_nobs,
        nobs_agree_pct: pct(same_nobs as u64, matched as u64),
        ref_observations: r.observations,
        matched_observations: matched_obs,
    }
}

fn build_df(rows: &[Row]) -> Result<DataFrame, Box<dyn Error>> {
    let df = df![
        "reference" => rows.iter().map(|r| r.reference.clone()).collect::<Vec<_>>(),
        "compared" => rows.iter().map(|r| r.compared.clone()).collect::<Vec<_>>(),
        "ref_platforms" => rows.iter().map(|r| r.ref_platforms).collect::<Vec<_>>(),
        "common_platforms" => rows.iter().map(|r| r.common_platforms).collect::<Vec<_>>(),
        "platform_cov_pct" => rows.iter().map(|r| r.platform_cov_pct).collect::<Vec<_>>(),
        "ref_profiles" => rows.iter().map(|r| r.ref_profiles).collect::<Vec<_>>(),
        "ref_unkeyed_profiles" => rows.iter().map(|r| r.ref_unkeyed_profiles).collect::<Vec<_>>(),
        "matched_profiles" => rows.iter().map(|r| r.matched_profiles).collect::<Vec<_>>(),
        "profile_cov_pct" => rows.iter().map(|r| r.profile_cov_pct).collect::<Vec<_>>(),
        "same_nobs" => rows.iter().map(|r| r.same_nobs).collect::<Vec<_>>(),
        "diff_nobs" => rows.iter().map(|r| r.diff_nobs).collect::<Vec<_>>(),
        "nobs_agree_pct" => rows.iter().map(|r| r.nobs_agree_pct).collect::<Vec<_>>(),
        "ref_observations" => rows.iter().map(|r| r.ref_observations).collect::<Vec<_>>(),
        "matched_observations" => rows.iter().map(|r| r.matched_observations).collect::<Vec<_>>(),
    ]?;
    Ok(df)
}

/// Label a file by its stem, which is what the `filename` column already holds.
fn label(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string())
}

/// Stream one file and reduce it to per-profile records.
fn scan_file(opts: &CompareOpts, src: &Path) -> Result<Scanned, Box<dyn Error>> {
    let scan = || {
        LazyFrame::scan_parquet(src, common::seq_scan_args())
            .map_err(|e| format!("Cannot scan {}: {}", src.display(), e))
    };

    // A zero-row collect is the cheapest way to the schema on this Polars version.
    let empty = scan()?.slice(0, 0).collect()?;
    let time_kind = check_columns(opts, &empty.schema(), src)?;

    let total = scan()?
        .select([len().alias("n")])
        .collect()?
        .column("n")?
        .u32()?
        .get(0)
        .unwrap_or(0) as usize;

    // Keyed by (platform, profile_no): the file's own profile identity.
    let mut profiles: HashMap<(String, u32), (Option<CmpKey>, u64)> = HashMap::new();
    let mut platforms: HashSet<String> = HashSet::new();

    let time_expr = match time_kind {
        TimeKind::Days1950 => col(opts.time_col.as_str()).cast(DataType::Float64).alias("_t"),
        TimeKind::Datetime => col(opts.time_col.as_str()).cast(DataType::Int64).alias("_t"),
    };

    let step = common::chunk_rows();
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let df = scan()?
            .slice(offset as i64, count as IdxSize)
            .select([
                col(opts.platform_col.as_str()).alias("_pc"),
                col("profile_no").alias("_pn"),
                time_expr.clone(),
                col(opts.lon_col.as_str()).cast(DataType::Float64).alias("_lon"),
                col(opts.lat_col.as_str()).cast(DataType::Float64).alias("_lat"),
            ])
            .collect()?;

        let pc = df.column("_pc")?.str()?;
        let pn = df.column("_pn")?.u32()?;
        let lon = df.column("_lon")?.f64()?;
        let lat = df.column("_lat")?.f64()?;
        // Read the time column at whichever width its dtype implies.
        let t_days = if time_kind == TimeKind::Days1950 { Some(df.column("_t")?.f64()?) } else { None };
        let t_ms = if time_kind == TimeKind::Datetime { Some(df.column("_t")?.i64()?) } else { None };

        for i in 0..df.height() {
            let platform = pc.get(i).unwrap_or("").to_string();
            let id = (platform.clone(), pn.get(i).unwrap_or(0));
            let entry = profiles.entry(id).or_insert_with(|| {
                let ms = match time_kind {
                    TimeKind::Days1950 => {
                        t_days.as_ref().and_then(|c| c.get(i)).and_then(days1950_to_ms)
                    }
                    TimeKind::Datetime => t_ms.as_ref().and_then(|c| c.get(i)),
                };
                let lo = lon.get(i).unwrap_or(f64::NAN);
                let la = lat.get(i).unwrap_or(f64::NAN);
                let base = opts.key.key(ms, lo, la);
                let key = base.map(|(time, x, y)| {
                    (opts.use_platform.then(|| platform.clone()), time, x, y)
                });
                (key, 0u64)
            });
            entry.1 += 1;
            platforms.insert(platform);
        }
        offset += count;
    }

    let observations = profiles.values().map(|(_, n)| *n).sum();
    Ok(Scanned {
        profiles: profiles.into_values().collect(),
        platforms,
        observations,
    })
}

/// Verify the named columns exist and decide how to read the time column. A
/// missing column is reported with the available ones, since the usual cause is
/// a misspelled `--*-col` or a file that is not a ctddump output.
fn check_columns(
    opts: &CompareOpts,
    schema: &Schema,
    src: &Path,
) -> Result<TimeKind, Box<dyn Error>> {
    let missing: Vec<&str> = [
        opts.platform_col.as_str(),
        "profile_no",
        opts.time_col.as_str(),
        opts.lon_col.as_str(),
        opts.lat_col.as_str(),
    ]
    .into_iter()
    .filter(|c| schema.get(c).is_none())
    .collect();

    if !missing.is_empty() {
        let available: Vec<String> = schema.iter_names().map(|n| n.to_string()).collect();
        return Err(format!(
            "{} has no column {} (available: {})",
            src.display(),
            missing.join(", "),
            available.join(", ")
        )
        .into());
    }

    match schema.get(opts.time_col.as_str()) {
        Some(DataType::Datetime(_, _)) => Ok(TimeKind::Datetime),
        Some(DataType::Float64) | Some(DataType::Float32) => Ok(TimeKind::Days1950),
        Some(other) => Err(format!(
            "{}: time column '{}' is {:?}, expected a datetime or a float of days since 1950",
            src.display(),
            opts.time_col,
            other
        )
        .into()),
        None => unreachable!("checked above"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days1950_matches_the_oceanographic_epoch() {
        // Day 0 is 1950-01-01T00:00:00Z.
        assert_eq!(days1950_to_ms(0.0), Some(EPOCH_1950_MS));
        // Day 7305 is 1970-01-01, the Unix epoch.
        assert_eq!(days1950_to_ms(7305.0), Some(0));
        // Half a day past day 0.
        assert_eq!(days1950_to_ms(0.5), Some(EPOCH_1950_MS + 43_200_000));
    }

    #[test]
    fn days1950_rejects_non_finite() {
        assert_eq!(days1950_to_ms(f64::NAN), None);
        assert_eq!(days1950_to_ms(f64::INFINITY), None);
    }

    #[test]
    fn pct_of_an_empty_denominator_is_nan() {
        assert!(pct(0, 0).is_nan());
        assert_eq!(pct(1, 4), 25.0);
        assert_eq!(pct(2, 3), 66.67);
    }
}
