//! Integration tests for the `dropqc` subcommand. Fixtures are built in-test
//! (no external test data required).

use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("dropqc should succeed");
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

/// Fixture exercising each keep/drop reason. `time_qc`/`position_qc` are
/// profile-level (constant within a profile); each profile spans two rows.
///   A/1 — time "1", pos "1"   both OK                 → keep
///   A/2 — time "1", pos ""    pos missing / NA        → keep
///   B/1 — time "", pos ""     file lacks profile QC   → keep
///   B/2 — time "4", pos "1"   time bad                → drop
///   C/1 — time "1", pos "0"   pos present, not OK     → drop
///   C/2 — time "9", pos "9"   flag "9" is NOT NA (-128 only) → drop
fn write_fixture(path: &Path) {
    let mut df = DataFrame::new(vec![
        Series::new(
            "platform_code".into(),
            vec!["A", "A", "A", "A", "B", "B", "B", "B", "C", "C", "C", "C"],
        ),
        Series::new(
            "profile_no".into(),
            vec![1u32, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2],
        ),
        Series::new(
            "observation_no".into(),
            vec![1u32, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2],
        ),
        //                            A/1        A/2       B/1       B/2       C/1       C/2
        Series::new("time_qc".into(), vec!["1", "1", "1", "1", "", "", "4", "4", "1", "1", "9", "9"]),
        Series::new("position_qc".into(), vec!["1", "1", "", "", "", "", "1", "1", "0", "0", "9", "9"]),
    ])
    .unwrap();
    let f = fs::File::create(path).unwrap();
    ParquetWriter::new(f).finish(&mut df).unwrap();
}

#[test]
fn test_dropqc_keeps_ok_and_missing() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    dispatch(&["dropqc", src.to_str().unwrap(), out.to_str().unwrap()]);

    assert_eq!(
        profiles(&out),
        vec![("A".to_string(), 1), ("A".to_string(), 2), ("B".to_string(), 1)]
    );
}

#[test]
fn test_dropqc_keeps_all_rows_of_kept_profile() {
    // A kept profile retains all its observations.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    dispatch(&["dropqc", src.to_str().unwrap(), out.to_str().unwrap()]);

    let df = ParquetReader::new(fs::File::open(&out).unwrap()).finish().unwrap();
    // 3 kept profiles × 2 obs each = 6 rows.
    assert_eq!(df.height(), 6);
}

#[test]
fn test_dropqc_streaming_chunk_independent() {
    // The QC predicate is per-row, so the result must not depend on chunk size.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    std::env::set_var("CTDDUMP_CHUNK_ROWS", "2");
    dispatch(&["dropqc", src.to_str().unwrap(), out.to_str().unwrap()]);
    std::env::remove_var("CTDDUMP_CHUNK_ROWS");

    assert_eq!(
        profiles(&out),
        vec![("A".to_string(), 1), ("A".to_string(), 2), ("B".to_string(), 1)]
    );
}

#[test]
fn test_dropqc_missing_qc_column_keeps_everything() {
    // A file whose profile QC is entirely missing ("" everywhere) loses nothing.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");

    let mut df = DataFrame::new(vec![
        Series::new("platform_code".into(), vec!["A", "A", "B", "B"]),
        Series::new("profile_no".into(), vec![1u32, 1, 1, 1]),
        Series::new("observation_no".into(), vec![1u32, 2, 1, 2]),
        Series::new("time_qc".into(), vec!["", "", "", ""]),
        Series::new("position_qc".into(), vec!["", "", "", ""]),
    ])
    .unwrap();
    ParquetWriter::new(fs::File::create(&src).unwrap()).finish(&mut df).unwrap();

    dispatch(&["dropqc", src.to_str().unwrap(), out.to_str().unwrap()]);

    assert_eq!(profiles(&out), vec![("A".to_string(), 1), ("B".to_string(), 1)]);
}

#[test]
fn test_dropqc_all_dropped_is_valid_empty() {
    // Every profile has a bad QC flag → a valid, empty Parquet file with schema.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");

    let mut df = DataFrame::new(vec![
        Series::new("platform_code".into(), vec!["A", "A"]),
        Series::new("profile_no".into(), vec![1u32, 1]),
        Series::new("observation_no".into(), vec![1u32, 2]),
        Series::new("time_qc".into(), vec!["4", "4"]),
        Series::new("position_qc".into(), vec!["1", "1"]),
    ])
    .unwrap();
    ParquetWriter::new(fs::File::create(&src).unwrap()).finish(&mut df).unwrap();

    dispatch(&["dropqc", src.to_str().unwrap(), out.to_str().unwrap()]);

    let out_df = ParquetReader::new(fs::File::open(&out).unwrap()).finish().unwrap();
    assert_eq!(out_df.height(), 0);
    assert!(out_df.get_column_names().iter().any(|c| c.as_str() == "time_qc"));
}
