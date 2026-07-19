//! `compare` under a tiny chunk size, where a profile's observation rows land in
//! different read slices and the per-profile reduction has to merge them.
//!
//! This file is its own test binary, so mutating the process-global
//! `CTDDUMP_CHUNK_ROWS` here does not affect the tests in `test_compare.rs`.

use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("command should succeed");
}

const DAY_2020_01_01: f64 = 25567.0;
const DAY_2020_01_02: f64 = 25568.0;
const DAY_2020_06_15: f64 = 25733.0;

fn ms_from_days(days: f64) -> i64 {
    (days * 86_400_000.0).round() as i64 - 631_152_000_000
}

type Profile = (&'static str, u32, usize, f64, f64, f64);

fn write_fixture(path: &Path, profiles: &[Profile]) {
    let mut platform = Vec::new();
    let mut profile_no = Vec::new();
    let mut observation_no = Vec::new();
    let mut profile_time = Vec::new();
    let mut ts_ms = Vec::new();
    let mut lon = Vec::new();
    let mut lat = Vec::new();

    for (pc, pn, n_obs, time, x, y) in profiles {
        for obs in 0..*n_obs {
            platform.push(*pc);
            profile_no.push(*pn);
            observation_no.push(obs as u32 + 1);
            profile_time.push(*time);
            ts_ms.push(ms_from_days(*time));
            lon.push(*x);
            lat.push(*y);
        }
    }
    let h = platform.len();

    let ts = Series::new("profile_timestamp".into(), ts_ms)
        .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
        .unwrap();
    let mut df = DataFrame::new(vec![
        Series::new("platform_code".into(), platform),
        Series::new("profile_no".into(), profile_no),
        Series::new("observation_no".into(), observation_no),
        Series::new("profile_time".into(), profile_time),
        ts,
        Series::new("longitude".into(), lon),
        Series::new("latitude".into(), lat),
        Series::new("temp".into(), vec![1.0f32; h]),
    ])
    .unwrap();
    ParquetWriter::new(fs::File::create(path).unwrap()).finish(&mut df).unwrap();
}

/// Profiles deliberately several observations deep, so a small chunk splits them.
fn profiles_a() -> Vec<Profile> {
    vec![
        ("P1", 1, 5, DAY_2020_01_01, 10.0, 60.0),
        ("P1", 2, 7, DAY_2020_01_02, 20.0, 70.0),
        ("P2", 1, 4, DAY_2020_06_15, 5.5, 55.5),
    ]
}

fn profiles_b() -> Vec<Profile> {
    vec![
        ("P1", 1, 5, DAY_2020_01_01, 10.0, 60.0), // matches, same n_obs
        ("P1", 2, 9, DAY_2020_01_02, 20.0, 70.0), // matches, different n_obs
        ("P4", 1, 4, DAY_2020_06_15, 5.5, 55.5),  // different platform
    ]
}

/// Run `compare` at a given chunk size and return the raw TSV.
fn compare_at(chunk_rows: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("file_a.parquet");
    let b = dir.path().join("file_b.parquet");
    let out = dir.path().join("cmp.tsv");
    write_fixture(&a, &profiles_a());
    write_fixture(&b, &profiles_b());

    std::env::set_var("CTDDUMP_CHUNK_ROWS", chunk_rows);
    dispatch(&["compare", a.to_str().unwrap(), b.to_str().unwrap(), out.to_str().unwrap()]);
    std::env::remove_var("CTDDUMP_CHUNK_ROWS");

    fs::read_to_string(&out).unwrap()
}

#[test]
fn a_tiny_chunk_gives_the_same_report_as_a_single_pass() {
    // One row per chunk splits every profile across slices.
    let split = compare_at("1");
    // Two rows per chunk splits them on different boundaries.
    let split_2 = compare_at("2");
    let whole = compare_at("1000000");

    assert_eq!(split, whole, "chunking must not change the comparison");
    assert_eq!(split_2, whole, "chunking must not change the comparison");
}

#[test]
fn observation_counts_survive_being_split_across_chunks() {
    let text = compare_at("1");
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let row: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let get = |name: &str| {
        let i = header.iter().position(|h| *h == name).unwrap();
        row[i]
    };

    // B as reference: 18 observations (5 + 9 + 4), 14 of them in matched profiles.
    assert_eq!(get("reference"), "file_b");
    assert_eq!(get("ref_observations"), "18");
    assert_eq!(get("matched_observations"), "14");
    // P1/1 agrees on 5 observations; P1/2 does not (7 vs 9).
    assert_eq!(get("matched_profiles"), "2");
    assert_eq!(get("same_nobs"), "1");
    assert_eq!(get("diff_nobs"), "1");
}
