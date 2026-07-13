//! Regression test for `markdup` on a multi-row-group **input**.
//!
//! `markdup`'s pass 2 collects each `chunk_rows()` slice and appends a freshly
//! built `is_dup` column. When a slice spans more than one input row group, the
//! scanned columns come back with several chunks while `is_dup` has a single
//! chunk; the Parquet `BatchedWriter` emits one Arrow record batch per chunk and
//! requires every column to share chunk boundaries. Without an explicit
//! `align_chunks` this panics with "RecordBatch requires all its arrays to have
//! an equal number of rows" and leaves a truncated output file.
//!
//! It lives in its own test binary so it can run with the default
//! `CTDDUMP_CHUNK_ROWS` (a single window covering the whole input) without
//! racing the `CTDDUMP_CHUNK_ROWS=1` mutation in `test_dedup_streaming.rs`.

use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("command should succeed");
}

/// Write a fixture whose Parquet has **many small row groups** (2 rows each), so
/// a single default-size read slice spans several of them.
fn write_multi_row_group_fixture(path: &Path) {
    // A/1 & B/1 duplicate (same date + 3-dp coords); unique C/1.
    let rows: &[(&str, u32, u32, i64, f64, f64)] = &[
        ("A", 1, 1, 1_577_836_800_000, 10.0001, 60.0001),
        ("A", 1, 2, 1_577_836_800_000, 10.0001, 60.0001),
        ("B", 1, 1, 1_577_858_400_000, 10.0004, 59.9996),
        ("B", 1, 2, 1_577_858_400_000, 10.0004, 59.9996),
        ("B", 1, 3, 1_577_858_400_000, 10.0004, 59.9996),
        ("C", 1, 1, 1_577_923_200_000, 30.0, 40.0),
    ];
    let h = rows.len();
    let ts = Series::new("profile_timestamp".into(), rows.iter().map(|r| r.3).collect::<Vec<_>>())
        .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
        .unwrap();
    let mut df = DataFrame::new(vec![
        Series::new("platform_code".into(), rows.iter().map(|r| r.0).collect::<Vec<_>>()),
        Series::new("profile_no".into(), rows.iter().map(|r| r.1).collect::<Vec<_>>()),
        Series::new("observation_no".into(), rows.iter().map(|r| r.2).collect::<Vec<_>>()),
        Series::new("profile_time".into(), rows.iter().map(|r| r.3 as f64).collect::<Vec<_>>()),
        ts,
        Series::new("longitude".into(), rows.iter().map(|r| r.4).collect::<Vec<_>>()),
        Series::new("latitude".into(), rows.iter().map(|r| r.5).collect::<Vec<_>>()),
        Series::new("temp".into(), vec![1.0f32; h]),
        Series::new("psal".into(), vec![1.0f32; h]),
        Series::new("pres".into(), vec![1.0f32; h]),
        Series::new("time_qc".into(), vec!["1"; h]),
        Series::new("position_qc".into(), vec!["1"; h]),
    ])
    .unwrap();
    // Two rows per row group → three row groups for six rows.
    ParquetWriter::new(fs::File::create(path).unwrap())
        .with_row_group_size(Some(2))
        .finish(&mut df)
        .unwrap();
}

#[test]
fn test_markdup_multi_row_group_input() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let marked = dir.path().join("marked.parquet");
    let dups = dir.path().join("dups.tsv");
    write_multi_row_group_fixture(&src);

    // Default chunk window covers the whole 6-row input in one slice, which spans
    // all three row groups — the case that used to panic in the batched writer.
    dispatch(&["markdup", src.to_str().unwrap(), marked.to_str().unwrap(), dups.to_str().unwrap()]);

    let m = ParquetReader::new(fs::File::open(&marked).unwrap()).finish().unwrap();
    assert_eq!(m.height(), 6, "all rows preserved");
    let pc = m.column("platform_code").unwrap().str().unwrap();
    let dup = m.column("is_dup").unwrap().bool().unwrap();
    for i in 0..m.height() {
        let expect = matches!(pc.get(i).unwrap(), "A" | "B");
        assert_eq!(dup.get(i).unwrap(), expect, "row {i} is_dup");
    }
}
