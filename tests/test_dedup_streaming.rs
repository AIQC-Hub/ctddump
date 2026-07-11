//! Multi-row-group streaming regression test for `markdup`/`dedup`.
//!
//! This lives in its own test binary (a separate process) because it mutates the
//! global `CTDDUMP_CHUNK_ROWS` env var; keeping it apart from the other dedup
//! tests avoids racing that setting into concurrently-running tests.
//!
//! With `CTDDUMP_CHUNK_ROWS=1`, `markdup` writes one row group per row, so its
//! output has many row groups. `dedup` then reads that file in tiny slices —
//! exactly the case Polars 0.43's parallel Parquet reader gets wrong (every
//! slice returns the first row group). `common::seq_scan_args()` disables the
//! parallel reader so the slices are correct; this test guards that fix.

use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("command should succeed");
}

fn write_fixture(path: &Path) {
    // A/1 & B/1 duplicate (same date + 3-dp coords), B/1 has more obs; unique C/1.
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
    ParquetWriter::new(fs::File::create(path).unwrap()).finish(&mut df).unwrap();
}

fn profiles(path: &Path) -> Vec<(String, u32)> {
    let df = ParquetReader::new(fs::File::open(path).unwrap()).finish().unwrap();
    let pc = df.column("platform_code").unwrap().str().unwrap();
    let pn = df.column("profile_no").unwrap().u32().unwrap();
    let mut v: Vec<(String, u32)> = pc
        .into_iter()
        .zip(pn)
        .map(|(c, n)| (c.unwrap().to_string(), n.unwrap()))
        .collect();
    v.sort();
    v.dedup();
    v
}

#[test]
fn test_markdup_then_dedup_stream_multi_row_group() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let marked = dir.path().join("marked.parquet");
    let dups = dir.path().join("dups.tsv");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    std::env::set_var("CTDDUMP_CHUNK_ROWS", "1"); // one row group per row
    dispatch(&["markdup", src.to_str().unwrap(), marked.to_str().unwrap(), dups.to_str().unwrap()]);

    // markdup marked the duplicates correctly despite the tiny chunks.
    let m = ParquetReader::new(fs::File::open(&marked).unwrap()).finish().unwrap();
    assert_eq!(m.height(), 6, "all rows preserved across the row groups");
    let pc = m.column("platform_code").unwrap().str().unwrap();
    let dup = m.column("is_dup").unwrap().bool().unwrap();
    for i in 0..m.height() {
        let expect = matches!(pc.get(i).unwrap(), "A" | "B");
        assert_eq!(dup.get(i).unwrap(), expect);
    }

    // dedup reads the many-row-group marked file in tiny slices and still keeps
    // the right profiles (B/1 over A/1; C/1 unique) — the slice-pushdown guard.
    dispatch(&["dedup", marked.to_str().unwrap(), out.to_str().unwrap()]);
    std::env::remove_var("CTDDUMP_CHUNK_ROWS");

    assert_eq!(profiles(&out), vec![("B".to_string(), 1), ("C".to_string(), 1)]);
    let od = ParquetReader::new(fs::File::open(&out).unwrap()).finish().unwrap();
    assert_eq!(od.column("is_dup").unwrap().bool().unwrap().sum().unwrap_or(0), 0);
}
