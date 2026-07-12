//! Multi-row-group streaming regression test for the slice-based commands
//! `dropqc`, `filter`, and `dropna`.
//!
//! Each of these commands streams its input in `CTDDUMP_CHUNK_ROWS` windows via
//! `scan().slice(offset, count)`. Polars 0.43's *parallel* Parquet reader
//! mishandles that slice once a file has enough row groups: every window returns
//! **row group 0**, so only the file's first row group survives and everything
//! after it is silently truncated. `common::seq_scan_args()` (parallel disabled)
//! is what makes the slices advance correctly; this test guards that fix.
//!
//! The unit-test fixtures elsewhere are written with `ParquetWriter::finish`
//! (a single row group), so they cannot exhibit the bug — hence this dedicated
//! many-row-group fixture. It lives in its own test binary (separate process)
//! because it mutates the global `CTDDUMP_CHUNK_ROWS` env var.

use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("command should succeed");
}

const N_PLAT: usize = 60; // one profile each, written as N_PLAT row groups

/// Distinct sorted platform codes in a Parquet file.
fn platforms(path: &Path) -> Vec<String> {
    let df = ParquetReader::new(fs::File::open(path).unwrap()).finish().unwrap();
    let c = df.column("platform_code").unwrap().str().unwrap();
    let mut v: Vec<String> = c.into_iter().flatten().map(|s| s.to_string()).collect();
    v.sort();
    v.dedup();
    v
}

/// Write `N_PLAT` platforms (`P00`..), 2 observations each, **one row group per
/// platform** (so the file has `N_PLAT` row groups — enough for Polars to engage
/// the buggy parallel reader). Platform `P30` carries a bad `position_qc` ("4");
/// every platform is otherwise inside the unit box with valid measurements.
fn write_fixture(path: &Path) {
    let row = |i: usize, obs: u32| {
        let pc = format!("P{i:02}");
        let pqc = if i == 30 { "4" } else { "1" };
        DataFrame::new(vec![
            Series::new("platform_code".into(), vec![pc]),
            Series::new("profile_no".into(), vec![1u32]),
            Series::new("observation_no".into(), vec![obs]),
            Series::new("longitude".into(), vec![10.0f64]),
            Series::new("latitude".into(), vec![50.0f64]),
            Series::new("temp".into(), vec![4.0f32]),
            Series::new("psal".into(), vec![35.0f32]),
            Series::new("pres".into(), vec![100.0f32]),
            Series::new("time_qc".into(), vec!["1"]),
            Series::new("position_qc".into(), vec![pqc]),
        ])
        .unwrap()
    };
    let schema = row(0, 1).schema();
    let mut w = ParquetWriter::new(fs::File::create(path).unwrap())
        .set_parallel(false)
        .batched(&schema)
        .unwrap();
    for i in 0..N_PLAT {
        // one row group per platform (2 obs)
        let mut df = row(i, 1).vstack(&row(i, 2)).unwrap();
        w.write_batch(&mut df).unwrap();
    }
    w.finish().unwrap();
}

/// All platforms except the one dropped for `except` (None = keep all).
fn expect_all_but(except: Option<usize>) -> Vec<String> {
    (0..N_PLAT).filter(|&i| Some(i) != except).map(|i| format!("P{i:02}")).collect()
}

#[test]
fn slice_commands_do_not_truncate_multi_row_group() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    write_fixture(&src);

    // A tiny chunk forces many streaming windows across the row groups.
    std::env::set_var("CTDDUMP_CHUNK_ROWS", "3");

    // dropqc: drops only the bad-QC platform (P30); the rest — including the last
    // platform, well past row group 0 — must survive.
    let qc = dir.path().join("qc.parquet");
    dispatch(&["dropqc", src.to_str().unwrap(), qc.to_str().unwrap()]);
    assert_eq!(platforms(&qc), expect_all_but(Some(30)), "dropqc truncated the file");

    // filter: a box covering every platform must keep all of them.
    let ft = dir.path().join("filter.parquet");
    dispatch(&[
        "filter", "--min-lon", "0", "--max-lon", "20", "--min-lat", "40", "--max-lat", "60",
        src.to_str().unwrap(), ft.to_str().unwrap(),
    ]);
    assert_eq!(platforms(&ft), expect_all_but(None), "filter truncated the file");

    // dropna: every profile has valid temp/psal/pres, so all must survive.
    let na = dir.path().join("dropna.parquet");
    dispatch(&["dropna", src.to_str().unwrap(), na.to_str().unwrap()]);
    assert_eq!(platforms(&na), expect_all_but(None), "dropna truncated the file");

    std::env::remove_var("CTDDUMP_CHUNK_ROWS");
}
