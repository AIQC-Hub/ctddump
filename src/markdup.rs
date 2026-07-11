//! Mark duplicate profiles with an `is_dup` column and write a report of them.
//!
//! A profile is a duplicate when its [`DupKey`] matches at least one *other*
//! profile's key (see [`crate::dupkey`]). Two streaming passes bound memory: pass
//! 1 reduces the file to one record per `(platform_code, profile_no)` profile,
//! counts distinct profiles per key, and writes the duplicates TSV; pass 2
//! re-streams the rows and appends the Boolean `is_dup` column.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::Write;
use std::path::Path;

use chrono::DateTime;
use polars::prelude::*;

use crate::convert::common;
use crate::dupkey::{DupKey, KeyOpts};

type ProfileId = (String, u32);

/// One record per profile, gathered in pass 1.
struct ProfileRec {
    key: Option<DupKey>,
    profile_time: f64,
    ts_ms: Option<i64>,
    lon: f64,
    lat: f64,
    n_obs: u64,
}

/// One row of the duplicates report.
struct DupRow {
    dup_group: u32,
    platform_code: String,
    profile_no: u32,
    profile_time: f64,
    ts_ms: Option<i64>,
    lon: f64,
    lat: f64,
    n_obs: u64,
}

pub fn run(opts: &KeyOpts, src: &Path, dest: &Path, dups: &Path) -> Result<(), Box<dyn Error>> {
    opts.validate()?;

    let scan = || {
        LazyFrame::scan_parquet(src, common::seq_scan_args())
            .map_err(|e| format!("Cannot scan {}: {}", src.display(), e))
    };

    let total = scan()?
        .select([len().alias("n")])
        .collect()?
        .column("n")?
        .u32()?
        .get(0)
        .unwrap_or(0) as usize;

    // ── Pass 1: one record per profile ──────────────────────────────────────
    let mut profiles: HashMap<ProfileId, ProfileRec> = HashMap::new();
    let step = common::chunk_rows();
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let df = scan()?
            .slice(offset as i64, count as IdxSize)
            .select([
                col("platform_code"),
                col("profile_no"),
                col("profile_time"),
                col("profile_timestamp").cast(DataType::Int64).alias("_ts"),
                col("longitude").cast(DataType::Float64).alias("_lon"),
                col("latitude").cast(DataType::Float64).alias("_lat"),
            ])
            .collect()?;
        let pc = df.column("platform_code")?.str()?;
        let pn = df.column("profile_no")?.u32()?;
        let pt = df.column("profile_time")?.f64()?;
        let ts = df.column("_ts")?.i64()?;
        let lon = df.column("_lon")?.f64()?;
        let lat = df.column("_lat")?.f64()?;
        for i in 0..df.height() {
            let id: ProfileId = (pc.get(i).unwrap_or("").to_string(), pn.get(i).unwrap_or(0));
            let rec = profiles.entry(id).or_insert_with(|| {
                let ts_ms = ts.get(i);
                let lo = lon.get(i).unwrap_or(f64::NAN);
                let la = lat.get(i).unwrap_or(f64::NAN);
                ProfileRec {
                    key: opts.key(ts_ms, lo, la),
                    profile_time: pt.get(i).unwrap_or(f64::NAN),
                    ts_ms,
                    lon: lo,
                    lat: la,
                    n_obs: 0,
                }
            });
            rec.n_obs += 1;
        }
        offset += count;
    }

    // Distinct profiles per key → which keys are duplicated (count > 1).
    let mut counts: HashMap<DupKey, u32> = HashMap::new();
    for rec in profiles.values() {
        if let Some(k) = &rec.key {
            *counts.entry(k.clone()).or_insert(0) += 1;
        }
    }
    // Assign a stable dup_group id to each duplicated key (sorted for determinism).
    let mut dup_keys: Vec<DupKey> =
        counts.iter().filter(|(_, &c)| c > 1).map(|(k, _)| k.clone()).collect();
    dup_keys.sort();
    let group_id: HashMap<DupKey, u32> =
        dup_keys.iter().enumerate().map(|(i, k)| (k.clone(), i as u32 + 1)).collect();

    // The set of duplicated profiles drives pass 2's is_dup column.
    let mut dup_profiles: HashSet<ProfileId> = HashSet::new();
    let mut report: Vec<DupRow> = Vec::new();
    for (id, rec) in &profiles {
        if let Some(k) = &rec.key {
            if let Some(&gid) = group_id.get(k) {
                dup_profiles.insert(id.clone());
                report.push(DupRow {
                    dup_group: gid,
                    platform_code: id.0.clone(),
                    profile_no: id.1,
                    profile_time: rec.profile_time,
                    ts_ms: rec.ts_ms,
                    lon: rec.lon,
                    lat: rec.lat,
                    n_obs: rec.n_obs,
                });
            }
        }
    }
    report.sort_by(|a, b| {
        (a.dup_group, &a.platform_code, a.profile_no).cmp(&(
            b.dup_group,
            &b.platform_code,
            b.profile_no,
        ))
    });
    write_report(dups, &report)?;

    // ── Pass 2: append the is_dup column ────────────────────────────────────
    let empty = scan()?
        .slice(0, 0)
        .collect()?
        .hstack(&[Series::new("is_dup".into(), Vec::<bool>::new())])?;
    let schema = empty.schema();
    let out = std::fs::File::create(dest)?;
    let mut writer = ParquetWriter::new(out).set_parallel(false).batched(&schema)?;

    let mut wrote_any = false;
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let mut df = scan()?.slice(offset as i64, count as IdxSize).collect()?;
        let flags: Vec<bool> = {
            let pc = df.column("platform_code")?.str()?;
            let pn = df.column("profile_no")?.u32()?;
            (0..df.height())
                .map(|i| {
                    let id: ProfileId = (pc.get(i).unwrap_or("").to_string(), pn.get(i).unwrap_or(0));
                    dup_profiles.contains(&id)
                })
                .collect()
        };
        df.with_column(Series::new("is_dup".into(), flags))?;
        if df.height() > 0 {
            writer.write_batch(&df)?;
            wrote_any = true;
        }
        offset += count;
    }
    if !wrote_any {
        writer.write_batch(&empty)?;
    }
    writer.finish()?;

    Ok(())
}

/// Write the duplicates TSV: one row per duplicated profile, grouped by dup_group.
fn write_report(path: &Path, rows: &[DupRow]) -> Result<(), Box<dyn Error>> {
    let mut f = std::fs::File::create(path)?;
    writeln!(
        f,
        "dup_group\tplatform_code\tprofile_no\tprofile_time\tprofile_timestamp\tlongitude\tlatitude\tn_obs"
    )?;
    for r in rows {
        let ts = r
            .ts_ms
            .and_then(DateTime::from_timestamp_millis)
            .map(|dt| dt.naive_utc().to_string())
            .unwrap_or_default();
        writeln!(
            f,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.dup_group, r.platform_code, r.profile_no, r.profile_time, ts, r.lon, r.lat, r.n_obs
        )?;
    }
    Ok(())
}
