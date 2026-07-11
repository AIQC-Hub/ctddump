//! Remove duplicate profiles, keeping one per duplicate group.
//!
//! Re-derives the same [`DupKey`] as `markdup` (so it can run standalone with the
//! same options) and, within each group of profiles sharing a key, keeps the one
//! with the most observation rows — ties broken by first appearance. Profiles
//! with no key (null timestamp / NaN position) are always kept. Two streaming
//! passes bound memory: pass 1 picks the winner of each key; pass 2 re-streams
//! and writes only the kept rows. If an `is_dup` column is present it is reset to
//! `false` (the survivors are unique), keeping the schema stable across the
//! `markdup → dedup` pipeline.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;

use polars::prelude::*;

use crate::convert::common;
use crate::dupkey::{DupKey, KeyOpts};

type ProfileId = (String, u32);

/// Per-profile info gathered in pass 1.
struct Rec {
    key: Option<DupKey>,
    n_obs: u64,
    order: u32,
}

pub fn run(opts: &KeyOpts, src: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
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

    // ── Pass 1: per-profile key, observation count, and first-seen order ─────
    let mut profiles: HashMap<ProfileId, Rec> = HashMap::new();
    let mut order: u32 = 0;
    let step = common::chunk_rows();
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let df = scan()?
            .slice(offset as i64, count as IdxSize)
            .select([
                col("platform_code"),
                col("profile_no"),
                col("profile_timestamp").cast(DataType::Int64).alias("_ts"),
                col("longitude").cast(DataType::Float64).alias("_lon"),
                col("latitude").cast(DataType::Float64).alias("_lat"),
            ])
            .collect()?;
        let pc = df.column("platform_code")?.str()?;
        let pn = df.column("profile_no")?.u32()?;
        let ts = df.column("_ts")?.i64()?;
        let lon = df.column("_lon")?.f64()?;
        let lat = df.column("_lat")?.f64()?;
        for i in 0..df.height() {
            let id: ProfileId = (pc.get(i).unwrap_or("").to_string(), pn.get(i).unwrap_or(0));
            let rec = profiles.entry(id).or_insert_with(|| {
                let o = order;
                order += 1;
                Rec {
                    key: opts.key(
                        ts.get(i),
                        lon.get(i).unwrap_or(f64::NAN),
                        lat.get(i).unwrap_or(f64::NAN),
                    ),
                    n_obs: 0,
                    order: o,
                }
            });
            rec.n_obs += 1;
        }
        offset += count;
    }

    // Winner per key: most observations, ties broken by earliest appearance.
    let mut best: HashMap<DupKey, (u64, u32, ProfileId)> = HashMap::new();
    let mut keep: HashSet<ProfileId> = HashSet::new();
    for (id, rec) in &profiles {
        match &rec.key {
            None => {
                keep.insert(id.clone()); // no key → never a duplicate
            }
            Some(k) => {
                let better = match best.get(k) {
                    None => true,
                    Some((n, o, _)) => rec.n_obs > *n || (rec.n_obs == *n && rec.order < *o),
                };
                if better {
                    best.insert(k.clone(), (rec.n_obs, rec.order, id.clone()));
                }
            }
        }
    }
    for (_, _, id) in best.values() {
        keep.insert(id.clone());
    }

    // ── Pass 2: keep only winners' rows; reset is_dup if present ─────────────
    let empty = scan()?.slice(0, 0).collect()?;
    let has_dup = empty.schema().index_of("is_dup").is_some();
    let schema = empty.schema();
    let out = std::fs::File::create(dest)?;
    let mut writer = ParquetWriter::new(out).set_parallel(false).batched(&schema)?;

    let mut wrote_any = false;
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let df = scan()?.slice(offset as i64, count as IdxSize).collect()?;
        let mask: BooleanChunked = {
            let pc = df.column("platform_code")?.str()?;
            let pn = df.column("profile_no")?.u32()?;
            (0..df.height())
                .map(|i| {
                    let id: ProfileId = (pc.get(i).unwrap_or("").to_string(), pn.get(i).unwrap_or(0));
                    Some(keep.contains(&id))
                })
                .collect()
        };
        let mut kept = df.filter(&mask)?;
        if has_dup && kept.height() > 0 {
            kept.with_column(Series::new("is_dup".into(), vec![false; kept.height()]))?;
        }
        if kept.height() > 0 {
            writer.write_batch(&kept)?;
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
