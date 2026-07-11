//! Drop-NA filter: remove whole profiles that are entirely NA in any of the
//! core measurement parameters (`temp`, `psal`, `pres`).
//!
//! A profile is kept only if **each** parameter has at least one non-NA
//! observation; it is dropped if **any one** parameter is all-NA across the
//! profile. Partial NAs within a parameter are fine.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;

use polars::prelude::*;

use crate::convert::common;

/// Core parameters that must each have at least one valid observation.
const PARAMS: [&str; 3] = ["temp", "psal", "pres"];

/// Filter `src`, dropping profiles that are all-NA in any of [`PARAMS`], and
/// write the survivors to `dest`.
///
/// Two streaming passes keep peak memory bounded like `convert`/`concat`:
/// pass 1 builds the keep-set of `(platform_code, profile_no)` by OR-ing each
/// chunk's per-profile "has ≥1 valid" flags (a hash group-by, so it is correct
/// even when a profile's rows straddle a chunk boundary); pass 2 re-streams the
/// file and writes only the rows whose profile is in the keep-set.
pub fn run(src: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
    let scan = || {
        LazyFrame::scan_parquet(src, common::seq_scan_args())
            .map_err(|e| format!("Cannot scan {}: {}", src.display(), e))
    };

    // Total input rows (from Parquet metadata) drives both streaming passes.
    let total = scan()?
        .select([len().alias("n")])
        .collect()?
        .column("n")?
        .u32()?
        .get(0)
        .unwrap_or(0) as usize;

    let keep = build_keep_set(&scan, total)?;

    let out_file = std::fs::File::create(dest)?;
    let empty = scan()?.slice(0, 0).collect()?;
    let schema = empty.schema();
    let mut writer = ParquetWriter::new(out_file).set_parallel(false).batched(&schema)?;

    let step = common::chunk_rows();
    let mut wrote_any = false;
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let df = scan()?.slice(offset as i64, count as IdxSize).collect()?;
        let mask = keep_mask(&df, &keep)?;
        let filtered = df.filter(&mask)?;
        if filtered.height() > 0 {
            writer.write_batch(&filtered)?;
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

/// Pass 1: for every profile, whether it has ≥1 valid observation in each
/// parameter, accumulated across all chunks. Returns, per `platform_code`, the
/// set of `profile_no` to keep (all parameters satisfied).
fn build_keep_set<F>(scan: &F, total: usize) -> Result<HashMap<String, HashSet<u32>>, Box<dyn Error>>
where
    F: Fn() -> Result<LazyFrame, String>,
{
    // Per-profile flags: [has_valid_temp, has_valid_psal, has_valid_pres].
    let mut flags: HashMap<(String, u32), [bool; 3]> = HashMap::new();

    let valid_count = |name: &str| {
        col(name)
            .is_not_null()
            .and(col(name).is_not_nan())
            .sum()
            .cast(DataType::Int64)
            .alias(name)
    };

    let step = common::chunk_rows();
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let partial = scan()?
            .slice(offset as i64, count as IdxSize)
            .group_by([col("platform_code"), col("profile_no")])
            .agg(PARAMS.iter().map(|p| valid_count(p)).collect::<Vec<_>>())
            .collect()?;

        let pc = partial.column("platform_code")?.str()?;
        let pn = partial.column("profile_no")?.u32()?;
        let counts: Vec<&Int64Chunked> = PARAMS
            .iter()
            .map(|p| partial.column(p).and_then(|c| c.i64()))
            .collect::<Result<_, _>>()?;

        for i in 0..partial.height() {
            let key = (pc.get(i).unwrap_or("").to_string(), pn.get(i).unwrap_or(0));
            let entry = flags.entry(key).or_default();
            for (j, cnt) in counts.iter().enumerate() {
                entry[j] |= cnt.get(i).unwrap_or(0) > 0;
            }
        }
        offset += count;
    }

    let mut keep: HashMap<String, HashSet<u32>> = HashMap::new();
    for ((platform, profile), f) in flags {
        if f.iter().all(|&ok| ok) {
            keep.entry(platform).or_default().insert(profile);
        }
    }
    Ok(keep)
}

/// Build a row mask selecting observations whose `(platform_code, profile_no)`
/// is in the keep-set.
fn keep_mask(df: &DataFrame, keep: &HashMap<String, HashSet<u32>>) -> PolarsResult<BooleanChunked> {
    let pc = df.column("platform_code")?.str()?;
    let pn = df.column("profile_no")?.u32()?;
    let mask: BooleanChunked = (0..df.height())
        .map(|i| match (pc.get(i), pn.get(i)) {
            (Some(p), Some(n)) => keep.get(p).is_some_and(|s| s.contains(&n)),
            _ => false,
        })
        .collect();
    Ok(mask)
}
