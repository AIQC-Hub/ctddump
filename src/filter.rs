//! Area filter: keep or drop whole profiles by a geographic bounding box.

use std::error::Error;
use std::path::Path;

use polars::prelude::*;

use crate::cli::FilterMode;
use crate::convert::common;

/// Filter `src` by the bounding box and write the result to `dest`.
///
/// The box is inclusive on all four edges. Because `longitude`/`latitude` are
/// constant within a profile (tiled from the profile coordinates), a plain row
/// predicate keeps or drops *whole profiles* without a group-by. A profile whose
/// position is NaN is treated as outside the box (dropped by `include`, kept by
/// `exclude`); the `is_not_nan` guards make that explicit regardless of how
/// Polars orders NaN in comparisons.
pub fn run(
    mode: FilterMode,
    min_lon: f64,
    max_lon: f64,
    min_lat: f64,
    max_lat: f64,
    src: &Path,
    dest: &Path,
) -> Result<(), Box<dyn Error>> {
    if min_lon > max_lon {
        return Err(format!("min-lon ({min_lon}) must not exceed max-lon ({max_lon})").into());
    }
    if min_lat > max_lat {
        return Err(format!("min-lat ({min_lat}) must not exceed max-lat ({max_lat})").into());
    }

    let inside = col("longitude")
        .is_not_nan()
        .and(col("latitude").is_not_nan())
        .and(col("longitude").gt_eq(lit(min_lon)))
        .and(col("longitude").lt_eq(lit(max_lon)))
        .and(col("latitude").gt_eq(lit(min_lat)))
        .and(col("latitude").lt_eq(lit(max_lat)));
    let predicate = match mode {
        FilterMode::Include => inside,
        FilterMode::Exclude => inside.not(),
    };

    let scan = || {
        LazyFrame::scan_parquet(src, ScanArgsParquet::default())
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
    // materialized at once. Slicing before filtering keeps each read bounded to
    // `chunk_rows()` input rows. A zero-row slice defines the schema up front so
    // an empty result is still a valid Parquet file. `set_parallel(false)` avoids
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
