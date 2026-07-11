//! QC filter: drop whole profiles whose profile-level QC is not usable.

use std::error::Error;
use std::path::Path;

use polars::prelude::*;

use crate::convert::common;

/// Drop profiles whose profile-level `time_qc` or `position_qc` is a present,
/// non-OK flag; write the survivors to `dest`.
///
/// A profile is kept iff *both* `time_qc` and `position_qc` are either `"1"`
/// (OK) or `""` (missing / NA — a `-128` byte or an absent QC variable both map
/// to `""`; see `common::i8_to_qc_string`). Any other flag (`"0"`, `"2"`..`"9"`,
/// or a non-numeric char code) drops the whole profile. Because these QC columns
/// are profile-level (constant within a profile), a plain row predicate keeps or
/// drops *whole profiles* without a group-by — the same streaming shape as the
/// area `filter`.
pub fn run(src: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
    // A profile-level QC is usable when it is OK ("1") or missing ("").
    let usable = |name: &str| col(name).eq(lit("1")).or(col(name).eq(lit("")));
    let predicate = usable("time_qc").and(usable("position_qc"));

    let scan = || {
        LazyFrame::scan_parquet(src, common::seq_scan_args())
            .map_err(|e| format!("Cannot scan {}: {}", src.display(), e))
    };

    // Total input rows (read from Parquet metadata) drives the streaming loop.
    let total = scan()?
        .select([len().alias("n")])
        .collect()?
        .column("n")?
        .u32()?
        .get(0)
        .unwrap_or(0) as usize;

    let out_file = std::fs::File::create(dest)?;

    // Stream the input in fixed row windows, filtering each window and writing it
    // as a Parquet row group so the whole (possibly large) file is never
    // materialized at once. A zero-row slice defines the schema up front so an
    // empty result is still a valid Parquet file. `set_parallel(false)` avoids
    // the parquet writer's leaky parallel-encoding path (see CLAUDE.md).
    let empty = scan()?.slice(0, 0).collect()?;
    let schema = empty.schema();
    let mut writer = ParquetWriter::new(out_file).set_parallel(false).batched(&schema)?;

    let step = common::chunk_rows();
    let mut wrote_any = false;
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let df = scan()?
            .slice(offset as i64, count as IdxSize)
            .filter(predicate.clone())
            .collect()?;
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
