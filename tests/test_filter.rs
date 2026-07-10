//! Integration tests for the `filter` subcommand. Fixtures are built in-test
//! (no external test data required).

use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("filter should succeed");
}

/// Small ctddump-shaped Parquet fixture with three profiles at distinct
/// positions, plus one profile whose position is NaN:
///   A/1 @ (0, 0)     — inside a box around the origin
///   A/2 @ (50, 50)   — outside
///   B/1 @ (10, 10)   — inside a wider box
///   B/2 @ (NaN, NaN) — unknown position
fn write_fixture(path: &Path) {
    let mut df = DataFrame::new(vec![
        Series::new("platform_code".into(), vec!["A", "A", "A", "A", "B", "B", "B"]),
        Series::new("profile_no".into(), vec![1u32, 1, 2, 2, 1, 1, 2]),
        Series::new("observation_no".into(), vec![1u32, 2, 1, 2, 1, 2, 1]),
        Series::new("longitude".into(), vec![0.0f32, 0.0, 50.0, 50.0, 10.0, 10.0, f32::NAN]),
        Series::new("latitude".into(), vec![0.0f32, 0.0, 50.0, 50.0, 10.0, 10.0, f32::NAN]),
        Series::new("temp".into(), vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]),
    ])
    .unwrap();
    let f = fs::File::create(path).unwrap();
    ParquetWriter::new(f).finish(&mut df).unwrap();
}

/// Return the sorted list of (platform, profile) pairs surviving in `path`.
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

#[test]
fn test_filter_area_include() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    // Box around the origin captures only A/1; A/2 and B/1 are outside; the
    // NaN-position B/2 is treated as outside and dropped.
    dispatch(&[
        "filter",
        "--min-lon", "-5", "--max-lon", "5",
        "--min-lat", "-5", "--max-lat", "5",
        src.to_str().unwrap(), out.to_str().unwrap(),
    ]);

    assert_eq!(profiles(&out), vec![("A".to_string(), 1)]);
}

#[test]
fn test_filter_area_exclude_keeps_nan() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    // Exclude the origin box: A/1 is dropped; A/2 and B/1 stay; the NaN-position
    // B/2 is treated as outside the box and therefore kept.
    dispatch(&[
        "filter", "--mode", "exclude",
        "--min-lon", "-5", "--max-lon", "5",
        "--min-lat", "-5", "--max-lat", "5",
        src.to_str().unwrap(), out.to_str().unwrap(),
    ]);

    assert_eq!(
        profiles(&out),
        vec![("A".to_string(), 2), ("B".to_string(), 1), ("B".to_string(), 2)]
    );
}

#[test]
fn test_filter_area_default_mode_is_include() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    // No --mode flag: defaults to include. A wide box captures A/1 and B/1.
    dispatch(&[
        "filter",
        "--min-lon", "-5", "--max-lon", "15",
        "--min-lat", "-5", "--max-lat", "15",
        src.to_str().unwrap(), out.to_str().unwrap(),
    ]);

    assert_eq!(profiles(&out), vec![("A".to_string(), 1), ("B".to_string(), 1)]);
}

#[test]
fn test_filter_area_streaming_is_chunk_independent() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    // Force many tiny row groups so the slice/filter loop runs multiple times;
    // the surviving profiles must match the single-chunk result exactly.
    std::env::set_var("CTDDUMP_CHUNK_ROWS", "2");
    dispatch(&[
        "filter",
        "--min-lon", "-5", "--max-lon", "15",
        "--min-lat", "-5", "--max-lat", "15",
        src.to_str().unwrap(), out.to_str().unwrap(),
    ]);
    std::env::remove_var("CTDDUMP_CHUNK_ROWS");

    assert_eq!(profiles(&out), vec![("A".to_string(), 1), ("B".to_string(), 1)]);
}

#[test]
fn test_filter_area_rejects_inverted_box() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    let err = handle_dispatch(
        &["filter", "--min-lon", "10", "--max-lon", "-10",
          "--min-lat", "0", "--max-lat", "5",
          src.to_str().unwrap(), out.to_str().unwrap()]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );
    assert!(err.is_err(), "min-lon > max-lon must be rejected");
}
