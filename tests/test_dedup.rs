//! Integration tests for the `markdup` and `dedup` subcommands and the
//! duplicate-aware `report`. Fixtures are built in-test (no external data).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("command should succeed");
}

// Unix ms for a few dates (00:00 UTC unless noted).
const D_2020_01_01: i64 = 1_577_836_800_000;
const D_2020_01_01_06H: i64 = 1_577_858_400_000; // same date, 06:00
const D_2020_01_02: i64 = 1_577_923_200_000;
const D_2020_06_15: i64 = 1_592_179_200_000;

/// Fixture with a known duplicate structure (default key = date + 3-dp coords):
///   A/1 (2 obs) & B/1 (3 obs)  — same date/rounded coords, cross-platform  → dup group 1
///   D/1 (2 obs) & E/1 (2 obs)  — same date/rounded coords, tie on n_obs    → dup group 2
///   A/2, C/1                   — unique                                    → not dup
///   F/1                        — NaN position (no key)                     → never dup
fn write_fixture(path: &Path) {
    let n = f32::NAN;
    // (platform, profile_no, obs_no, ts_ms, lon, lat)
    let rows: &[(&str, u32, u32, i64, f64, f64)] = &[
        ("A", 1, 1, D_2020_01_01, 10.0001, 60.0001),
        ("A", 1, 2, D_2020_01_01, 10.0001, 60.0001),
        ("B", 1, 1, D_2020_01_01_06H, 10.0004, 59.9996),
        ("B", 1, 2, D_2020_01_01_06H, 10.0004, 59.9996),
        ("B", 1, 3, D_2020_01_01_06H, 10.0004, 59.9996),
        ("A", 2, 1, D_2020_01_02, 20.0, 70.0),
        ("C", 1, 1, D_2020_01_01, 30.0, 40.0),
        ("D", 1, 1, D_2020_06_15, 5.5, 55.5),
        ("D", 1, 2, D_2020_06_15, 5.5, 55.5),
        ("E", 1, 1, D_2020_06_15, 5.5001, 55.4999),
        ("E", 1, 2, D_2020_06_15, 5.5001, 55.4999),
        ("F", 1, 1, D_2020_01_01, f64::NAN, f64::NAN),
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
        Series::new("temp_qc".into(), vec!["1"; h]),
        Series::new("time_qc".into(), vec!["1"; h]),
        Series::new("position_qc".into(), vec!["1"; h]),
    ])
    .unwrap();
    let _ = n;
    ParquetWriter::new(fs::File::create(path).unwrap()).finish(&mut df).unwrap();
}

/// Read a marked file's per-profile is_dup flag into a map keyed by (platform, profile_no).
fn dup_flags(path: &Path) -> HashMap<(String, u32), bool> {
    let df = ParquetReader::new(fs::File::open(path).unwrap()).finish().unwrap();
    let pc = df.column("platform_code").unwrap().str().unwrap();
    let pn = df.column("profile_no").unwrap().u32().unwrap();
    let dup = df.column("is_dup").unwrap().bool().unwrap();
    let mut m = HashMap::new();
    for i in 0..df.height() {
        m.insert(
            (pc.get(i).unwrap().to_string(), pn.get(i).unwrap()),
            dup.get(i).unwrap(),
        );
    }
    m
}

/// Sorted, de-duplicated surviving (platform, profile) pairs.
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
fn test_markdup_flags_and_report() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let marked = dir.path().join("marked.parquet");
    let dups = dir.path().join("dups.tsv");
    write_fixture(&src);

    dispatch(&["markdup", src.to_str().unwrap(), marked.to_str().unwrap(), dups.to_str().unwrap()]);

    let f = dup_flags(&marked);
    for id in [("A", 1), ("B", 1), ("D", 1), ("E", 1)] {
        assert!(f[&(id.0.to_string(), id.1)], "{id:?} should be a duplicate");
    }
    for id in [("A", 2), ("C", 1), ("F", 1)] {
        assert!(!f[&(id.0.to_string(), id.1)], "{id:?} should not be a duplicate");
    }

    // Duplicates TSV: header + 4 rows, grouped and sorted.
    let text = fs::read_to_string(&dups).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0].split('\t').next().unwrap(), "dup_group");
    let got: Vec<(&str, &str, &str)> = lines[1..]
        .iter()
        .map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c[0], c[1], c[2]) // dup_group, platform_code, profile_no
        })
        .collect();
    assert_eq!(
        got,
        vec![("1", "A", "1"), ("1", "B", "1"), ("2", "D", "1"), ("2", "E", "1")]
    );
}

#[test]
fn test_report_counts_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let marked = dir.path().join("marked.parquet");
    let dups = dir.path().join("dups.tsv");
    let rpt = dir.path().join("report.tsv");
    write_fixture(&src);
    dispatch(&["markdup", src.to_str().unwrap(), marked.to_str().unwrap(), dups.to_str().unwrap()]);

    dispatch(&["report", "parquet", "--level", "global", marked.to_str().unwrap(), rpt.to_str().unwrap()]);
    let text = fs::read_to_string(&rpt).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let header: Vec<&str> = lines[0].split('\t').collect();
    let vals: Vec<&str> = lines[1].split('\t').collect();
    let idx = header.iter().position(|h| *h == "dup_profiles").expect("dup_profiles column");
    assert_eq!(vals[idx], "4");
}

#[test]
fn test_report_without_dup_column_omits_it() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let rpt = dir.path().join("report.tsv");
    write_fixture(&src);

    dispatch(&["report", "parquet", "--level", "global", src.to_str().unwrap(), rpt.to_str().unwrap()]);
    let text = fs::read_to_string(&rpt).unwrap();
    assert!(!text.lines().next().unwrap().contains("dup_profiles"));
}

#[test]
fn test_dedup_keeps_best_of_each_group() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);

    dispatch(&["dedup", src.to_str().unwrap(), out.to_str().unwrap()]);

    // B/1 (3 obs) beats A/1 (2); D/1 kept over E/1 by first appearance (tie);
    // A/2, C/1, F/1 (no key) all survive.
    assert_eq!(
        profiles(&out),
        vec![
            ("A".to_string(), 2),
            ("B".to_string(), 1),
            ("C".to_string(), 1),
            ("D".to_string(), 1),
            ("F".to_string(), 1),
        ]
    );
}

#[test]
fn test_dedup_after_markdup_resets_is_dup() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let marked = dir.path().join("marked.parquet");
    let dups = dir.path().join("dups.tsv");
    let out = dir.path().join("out.parquet");
    write_fixture(&src);
    dispatch(&["markdup", src.to_str().unwrap(), marked.to_str().unwrap(), dups.to_str().unwrap()]);

    dispatch(&["dedup", marked.to_str().unwrap(), out.to_str().unwrap()]);

    // Schema kept (is_dup present) and every survivor is now unique → is_dup false.
    let df = ParquetReader::new(fs::File::open(&out).unwrap()).finish().unwrap();
    let dup = df.column("is_dup").unwrap().bool().unwrap();
    assert_eq!(dup.sum().unwrap_or(0), 0);
    assert_eq!(profiles(&out).len(), 5);
}

