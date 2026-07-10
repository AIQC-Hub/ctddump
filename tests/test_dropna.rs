//! Integration tests for the `dropna` subcommand. Fixtures are built in-test
//! (no external test data required).

use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("dropna should succeed");
}

/// Return the sorted, de-duplicated list of surviving (platform, profile) pairs.
fn profiles(path: &Path) -> Vec<(String, u32)> {
    let df = ParquetReader::new(fs::File::open(path).unwrap()).finish().unwrap();
    let pc = df.column("platform_code").unwrap().str().unwrap();
    let pn = df.column("profile_no").unwrap().u32().unwrap();
    let mut pairs: Vec<(String, u32)> = pc
        .into_iter()
        .zip(pn)
        .map(|(c, n)| (c.unwrap().to_string(), n.unwrap()))
        .collect();
    pairs.sort();
    pairs.dedup();
    pairs
}

/// Fixture exercising each drop reason. Columns are observation-level; each
/// profile spans consecutive rows.
///   A/1 — temp partly NaN, psal all present, pres present   → keep
///   A/2 — psal ALL NaN (temp/pres fine)                     → drop
///   B/1 — temp ALL NaN                                      → drop
///   B/2 — pres ALL NaN                                      → drop
///   C/1 — every param has ≥1 valid (all partly NaN)         → keep
fn write_fixture(path: &Path) {
    let n = f32::NAN;
    let mut df = DataFrame::new(vec![
        Series::new(
            "platform_code".into(),
            vec!["A", "A", "A", "A", "B", "B", "B", "B", "C", "C"],
        ),
        Series::new("profile_no".into(), vec![1u32, 1, 2, 2, 1, 1, 2, 2, 1, 1]),
        Series::new("observation_no".into(), vec![1u32, 2, 1, 2, 1, 2, 1, 2, 1, 2]),
        //                         A/1        A/2       B/1       B/2       C/1
        Series::new("temp".into(), vec![n, 2.0, 3.0, 4.0, n, n, 7.0, 8.0, 9.0, n]),
        Series::new("psal".into(), vec![10.0, 11.0, n, n, 12.0, 13.0, 14.0, 15.0, n, 16.0]),
        Series::new("pres".into(), vec![0.0, 5.0, 0.0, 5.0, 0.0, 5.0, n, n, 0.0, n]),
    ])
    .unwrap();
    let f = fs::File::create(path).unwrap();
    ParquetWriter::new(f).finish(&mut df).unwrap();
}

#[test]
fn test_dropna_drops_all_na_params() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    dispatch(&["dropna", src.to_str().unwrap(), out.to_str().unwrap()]);

    assert_eq!(profiles(&out), vec![("A".to_string(), 1), ("C".to_string(), 1)]);
}

#[test]
fn test_dropna_preserves_partial_na_rows() {
    // A kept profile must retain ALL its observations, including the NaN ones.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    dispatch(&["dropna", src.to_str().unwrap(), out.to_str().unwrap()]);

    let df = ParquetReader::new(fs::File::open(&out).unwrap()).finish().unwrap();
    // A/1 (2 obs) + C/1 (2 obs) survive = 4 rows; nothing within a kept profile
    // is dropped even though several values are NaN.
    assert_eq!(df.height(), 4);
}

#[test]
fn test_dropna_streaming_merges_across_chunks() {
    // The key streaming-correctness case: force tiny chunks so a profile's rows
    // split across chunk boundaries. C/1's only valid psal is in its 2nd row and
    // its only valid temp in its 1st — a per-chunk check would wrongly drop it;
    // the pass-1 merge must keep it.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    std::env::set_var("CTDDUMP_CHUNK_ROWS", "1");
    dispatch(&["dropna", src.to_str().unwrap(), out.to_str().unwrap()]);
    std::env::remove_var("CTDDUMP_CHUNK_ROWS");

    assert_eq!(profiles(&out), vec![("A".to_string(), 1), ("C".to_string(), 1)]);
}

#[test]
fn test_dropna_all_dropped_is_valid_empty() {
    // Every profile is all-NA in some parameter → a valid, empty Parquet file.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");

    let n = f32::NAN;
    let mut df = DataFrame::new(vec![
        Series::new("platform_code".into(), vec!["A", "A"]),
        Series::new("profile_no".into(), vec![1u32, 1]),
        Series::new("observation_no".into(), vec![1u32, 2]),
        Series::new("temp".into(), vec![n, n]), // all-NaN temp
        Series::new("psal".into(), vec![10.0f32, 11.0]),
        Series::new("pres".into(), vec![0.0f32, 5.0]),
    ])
    .unwrap();
    ParquetWriter::new(fs::File::create(&src).unwrap()).finish(&mut df).unwrap();

    dispatch(&["dropna", src.to_str().unwrap(), out.to_str().unwrap()]);

    let out_df = ParquetReader::new(fs::File::open(&out).unwrap()).finish().unwrap();
    assert_eq!(out_df.height(), 0);
    // Schema is preserved so downstream tools still see the columns.
    assert!(out_df.get_column_names().iter().any(|c| c.as_str() == "temp"));
}
