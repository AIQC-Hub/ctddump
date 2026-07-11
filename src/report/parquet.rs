//! Parquet report: aggregate a ctddump Parquet file at the global, platform, or
//! profile level and emit a summary table.

use std::collections::HashSet;
use std::error::Error;
use std::path::Path;

use polars::prelude::*;

use crate::cli::{ReportFormat, ReportLevel};
use crate::report::format;

/// Summarise `src` at `level` and write the report in `format` to `dest` (or stdout).
pub fn run(
    level: ReportLevel,
    format: ReportFormat,
    src: &Path,
    dest: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let lf = LazyFrame::scan_parquet(src, ScanArgsParquet::default())
        .map_err(|e| format!("Cannot scan {}: {}", src.display(), e))?;

    // When a `markdup` `is_dup` column is present, the report also summarises
    // duplicate counts; otherwise the duplicate columns are omitted entirely.
    let has_dup = lf.clone().slice(0, 0).collect()?.schema().index_of("is_dup").is_some();

    let df = match level {
        ReportLevel::Global => global_report(lf, has_dup)?,
        ReportLevel::Platform => platform_report(lf, has_dup)?,
        ReportLevel::Profile => profile_report(lf, has_dup)?,
    };

    format::write_report(&df, format, dest)?;
    Ok(())
}

/// Observation-level aggregates for `temp`/`psal`/`pres`: NA count (null or NaN)
/// plus min/max/mean over the valid (non-NaN) values. Reused by every level.
fn measure_aggs() -> Vec<Expr> {
    let mut v = Vec::new();
    for name in ["temp", "psal", "pres"] {
        v.push(
            col(name)
                .is_null()
                .or(col(name).is_nan())
                .sum()
                .alias(format!("na_{name}").as_str()),
        );
        let valid = col(name).filter(col(name).is_not_nan());
        v.push(valid.clone().min().alias(format!("{name}_min").as_str()));
        v.push(valid.clone().max().alias(format!("{name}_max").as_str()));
        v.push(valid.mean().alias(format!("{name}_mean").as_str()));
    }
    v
}

/// Columns in report order, after the level-specific identifier columns.
fn measure_cols() -> Vec<Expr> {
    let mut v = Vec::new();
    for name in ["temp", "psal", "pres"] {
        v.push(col(format!("na_{name}").as_str()));
        v.push(col(format!("{name}_min").as_str()));
        v.push(col(format!("{name}_max").as_str()));
        v.push(col(format!("{name}_mean").as_str()));
    }
    v
}

/// Geographic bounding-box aggregates: min/max of `longitude` and `latitude`
/// over the valid (non-NaN) positions. Used by the global and platform levels.
fn geo_aggs() -> Vec<Expr> {
    let mut v = Vec::new();
    for name in ["longitude", "latitude"] {
        let valid = col(name).filter(col(name).is_not_nan());
        v.push(valid.clone().min().alias(format!("{name}_min").as_str()));
        v.push(valid.max().alias(format!("{name}_max").as_str()));
    }
    v
}

fn geo_cols() -> Vec<Expr> {
    let mut v = Vec::new();
    for name in ["longitude", "latitude"] {
        v.push(col(format!("{name}_min").as_str()));
        v.push(col(format!("{name}_max").as_str()));
    }
    v
}

/// Per-profile roll-up of the profile-level QC flags into per-platform "good"
/// counts (flag `== "1"`), plus the profile count. With `has_dup`, also counts
/// duplicated profiles per platform (`is_dup` is profile-level).
fn platform_qc(lf: LazyFrame, has_dup: bool) -> LazyFrame {
    let mut prof_agg = vec![col("time_qc").first(), col("position_qc").first()];
    let mut plat_agg = vec![
        len().alias("n_profiles"),
        col("time_qc").eq(lit("1")).sum().alias("time_qc_good"),
        col("position_qc").eq(lit("1")).sum().alias("position_qc_good"),
    ];
    if has_dup {
        prof_agg.push(col("is_dup").first());
        plat_agg.push(col("is_dup").sum().alias("dup_profiles"));
    }
    lf.group_by([col("platform_code"), col("profile_no")])
        .agg(prof_agg)
        .group_by([col("platform_code")])
        .agg(plat_agg)
}

fn platform_report(lf: LazyFrame, has_dup: bool) -> Result<DataFrame, Box<dyn Error>> {
    let obs = lf.clone().group_by([col("platform_code")]).agg({
        let mut v = vec![len().alias("n_obs")];
        v.extend(measure_aggs());
        v.extend(geo_aggs());
        v
    });
    let qc = platform_qc(lf, has_dup);

    let mut select_cols = vec![
        col("platform_code"),
        col("n_profiles"),
        col("n_obs"),
        col("time_qc_good"),
        col("position_qc_good"),
    ];
    if has_dup {
        select_cols.push(col("dup_profiles"));
    }
    select_cols.extend(geo_cols());
    select_cols.extend(measure_cols());

    let df = obs
        .join(
            qc,
            [col("platform_code")],
            [col("platform_code")],
            JoinArgs::new(JoinType::Left),
        )
        .sort(vec!["platform_code"], SortMultipleOptions::default())
        .select(select_cols)
        .collect()?;
    Ok(df)
}

fn profile_report(lf: LazyFrame, has_dup: bool) -> Result<DataFrame, Box<dyn Error>> {
    let agg = {
        let mut v = vec![
            col("profile_timestamp").first().alias("profile_timestamp"),
            col("longitude").first().alias("longitude"),
            col("latitude").first().alias("latitude"),
            len().alias("n_obs"),
            col("time_qc").first().alias("time_qc"),
            col("position_qc").first().alias("position_qc"),
        ];
        if has_dup {
            v.push(col("is_dup").first().alias("is_dup"));
        }
        v.extend(measure_aggs());
        v
    };

    let mut select_cols = vec![
        col("platform_code"),
        col("profile_no"),
        col("profile_timestamp"),
        col("longitude"),
        col("latitude"),
        col("n_obs"),
        col("time_qc"),
        col("position_qc"),
    ];
    if has_dup {
        select_cols.push(col("is_dup"));
    }
    select_cols.extend(measure_cols());

    let df = lf
        .group_by([col("platform_code"), col("profile_no")])
        .agg(agg)
        .sort(
            vec!["platform_code", "profile_no"],
            SortMultipleOptions::default(),
        )
        .select(select_cols)
        .collect()?;
    Ok(df)
}

fn global_report(lf: LazyFrame, has_dup: bool) -> Result<DataFrame, Box<dyn Error>> {
    // Whole-file observation aggregates (one row).
    let base = lf
        .clone()
        .select({
            let mut v = vec![len().alias("n_obs")];
            v.extend(measure_aggs());
            v.extend(geo_aggs());
            v
        })
        .collect()?;

    // Profile-level reduction for platform/profile counts and QC "good" counts.
    let profiles = lf
        .group_by([col("platform_code"), col("profile_no")])
        .agg({
            let mut v = vec![col("time_qc").first(), col("position_qc").first()];
            if has_dup {
                v.push(col("is_dup").first());
            }
            v
        })
        .collect()?;

    let n_profiles = profiles.height() as u32;
    let time_qc_good = count_eq(profiles.column("time_qc")?.str()?, "1");
    let position_qc_good = count_eq(profiles.column("position_qc")?.str()?, "1");

    let mut platforms = HashSet::new();
    for s in profiles.column("platform_code")?.str()?.into_iter().flatten() {
        platforms.insert(s.to_string());
    }
    let n_platforms = platforms.len() as u32;

    let mut cols: Vec<Series> = vec![
        Series::new("n_platforms".into(), vec![n_platforms]),
        Series::new("n_profiles".into(), vec![n_profiles]),
        base.column("n_obs")?.clone(),
        Series::new("time_qc_good".into(), vec![time_qc_good]),
        Series::new("position_qc_good".into(), vec![position_qc_good]),
    ];
    if has_dup {
        let dup_profiles = profiles.column("is_dup")?.bool()?.sum().unwrap_or(0);
        cols.push(Series::new("dup_profiles".into(), vec![dup_profiles]));
    }
    for name in ["longitude", "latitude"] {
        cols.push(base.column(format!("{name}_min").as_str())?.clone());
        cols.push(base.column(format!("{name}_max").as_str())?.clone());
    }
    for name in ["temp", "psal", "pres"] {
        cols.push(base.column(format!("na_{name}").as_str())?.clone());
        cols.push(base.column(format!("{name}_min").as_str())?.clone());
        cols.push(base.column(format!("{name}_max").as_str())?.clone());
        cols.push(base.column(format!("{name}_mean").as_str())?.clone());
    }
    Ok(DataFrame::new(cols)?)
}

/// Count entries equal to `val` in a string column.
fn count_eq(ca: &StringChunked, val: &str) -> u32 {
    ca.into_iter().filter(|o| *o == Some(val)).count() as u32
}
