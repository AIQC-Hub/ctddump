//! Integration tests for the `compare` subcommand. Fixtures are built in-test
//! (no external data).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("command should succeed");
}

// `profile_time` is days since 1950-01-01. Day 7305 is the Unix epoch.
const DAY_2020_01_01: f64 = 25567.0;
const DAY_2020_01_01_06H: f64 = 25567.25; // same date, 06:00
const DAY_2020_01_02: f64 = 25568.0;
const DAY_2020_06_15: f64 = 25733.0;

/// Unix ms for a `profile_time` in days since 1950, so both time columns in a
/// fixture describe the same instant.
fn ms_from_days(days: f64) -> i64 {
    (days * 86_400_000.0).round() as i64 - 631_152_000_000
}

/// One profile: platform, profile_no, observation count, time, lon, lat.
type Profile = (&'static str, u32, usize, f64, f64, f64);

/// File A:
///   P1/1  2 obs  2020-01-01        10.0001 / 60.0001
///   P1/2  3 obs  2020-01-02        20.0    / 70.0
///   P2/1  2 obs  2020-06-15         5.5    / 55.5
///   P3/1  1 obs  NaN position (never matches)
fn profiles_a() -> Vec<Profile> {
    vec![
        ("P1", 1, 2, DAY_2020_01_01, 10.0001, 60.0001),
        ("P1", 2, 3, DAY_2020_01_02, 20.0, 70.0),
        ("P2", 1, 2, DAY_2020_06_15, 5.5, 55.5),
        ("P3", 1, 1, DAY_2020_01_01, f64::NAN, f64::NAN),
    ]
}

/// File B:
///   P1/1  2 obs  2020-01-01 06:00  10.0004 / 59.9996  → matches A P1/1, same n_obs
///   P1/2  5 obs  2020-01-02        20.0    / 70.0     → matches A P1/2, different n_obs
///   P4/1  2 obs  2020-06-15         5.5    / 55.5     → same time/position as A P2/1,
///                                                       different platform
fn profiles_b() -> Vec<Profile> {
    vec![
        ("P1", 1, 2, DAY_2020_01_01_06H, 10.0004, 59.9996),
        ("P1", 2, 5, DAY_2020_01_02, 20.0, 70.0),
        ("P4", 1, 2, DAY_2020_06_15, 5.5, 55.5),
    ]
}

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
        Series::new("psal".into(), vec![1.0f32; h]),
        Series::new("pres".into(), vec![1.0f32; h]),
    ])
    .unwrap();
    ParquetWriter::new(fs::File::create(path).unwrap()).finish(&mut df).unwrap();
}

/// Parse the TSV report into one map per row, in output order.
fn read_tsv(path: &Path) -> Vec<HashMap<String, String>> {
    let text = fs::read_to_string(path).unwrap();
    let mut lines = text.lines();
    let header: Vec<String> = lines.next().unwrap().split('\t').map(|s| s.to_string()).collect();
    lines
        .map(|line| {
            header.iter().cloned().zip(line.split('\t').map(|s| s.to_string())).collect()
        })
        .collect()
}

/// Build both fixtures in a temp dir and run `compare`, returning the parsed rows.
fn run_compare(extra: &[&str]) -> (tempfile::TempDir, Vec<HashMap<String, String>>) {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("file_a.parquet");
    let b = dir.path().join("file_b.parquet");
    let out = dir.path().join("cmp.tsv");
    write_fixture(&a, &profiles_a());
    write_fixture(&b, &profiles_b());

    let mut args: Vec<&str> = vec!["compare"];
    args.extend_from_slice(extra);
    let (a_s, b_s, out_s) = (
        a.to_str().unwrap().to_string(),
        b.to_str().unwrap().to_string(),
        out.to_str().unwrap().to_string(),
    );
    args.push(&a_s);
    args.push(&b_s);
    args.push(&out_s);
    dispatch(&args);

    let rows = read_tsv(&out);
    (dir, rows)
}

fn get<'a>(row: &'a HashMap<String, String>, k: &str) -> &'a str {
    row.get(k).unwrap_or_else(|| panic!("missing column {k}")).as_str()
}

#[test]
fn reports_both_directions_with_the_second_file_first() {
    let (_dir, rows) = run_compare(&[]);
    assert_eq!(rows.len(), 2, "one row per direction");
    // The second file is the reference in the first row.
    assert_eq!(get(&rows[0], "reference"), "file_b");
    assert_eq!(get(&rows[0], "compared"), "file_a");
    assert_eq!(get(&rows[1], "reference"), "file_a");
    assert_eq!(get(&rows[1], "compared"), "file_b");
}

#[test]
fn coverage_is_asymmetric_between_the_two_directions() {
    let (_dir, rows) = run_compare(&[]);

    // B as reference: 3 profiles, 2 of them present in A.
    let b = &rows[0];
    assert_eq!(get(b, "ref_platforms"), "2"); // P1, P4
    assert_eq!(get(b, "common_platforms"), "1"); // P1
    assert_eq!(get(b, "platform_cov_pct"), "50");
    assert_eq!(get(b, "ref_profiles"), "3");
    assert_eq!(get(b, "ref_unkeyed_profiles"), "0");
    assert_eq!(get(b, "matched_profiles"), "2");
    assert_eq!(get(b, "profile_cov_pct"), "66.67");
    assert_eq!(get(b, "ref_observations"), "9"); // 2 + 5 + 2
    assert_eq!(get(b, "matched_observations"), "7"); // 2 + 5

    // A as reference: 4 profiles (one unkeyed), 2 of them present in B.
    let a = &rows[1];
    assert_eq!(get(a, "ref_platforms"), "3"); // P1, P2, P3
    assert_eq!(get(a, "common_platforms"), "1");
    assert_eq!(get(a, "platform_cov_pct"), "33.33");
    assert_eq!(get(a, "ref_profiles"), "4");
    assert_eq!(get(a, "ref_unkeyed_profiles"), "1"); // NaN position
    assert_eq!(get(a, "matched_profiles"), "2");
    assert_eq!(get(a, "profile_cov_pct"), "50");
    assert_eq!(get(a, "ref_observations"), "8"); // 2 + 3 + 2 + 1
    assert_eq!(get(a, "matched_observations"), "5"); // 2 + 3
}

#[test]
fn matched_profiles_are_split_by_observation_count_agreement() {
    let (_dir, rows) = run_compare(&[]);
    for row in &rows {
        // P1/1 agrees (2 vs 2); P1/2 does not (3 vs 5).
        assert_eq!(get(row, "same_nobs"), "1");
        assert_eq!(get(row, "diff_nobs"), "1");
        assert_eq!(get(row, "nobs_agree_pct"), "50");
    }
}

#[test]
fn a_date_key_matches_profiles_at_different_times_of_day() {
    // A P1/1 is at 00:00 and B P1/1 at 06:00 on the same date, and they match
    // under the default date-only key.
    let (_dir, rows) = run_compare(&[]);
    assert_eq!(get(&rows[0], "matched_profiles"), "2");

    // With the time of day in the key they no longer match, leaving only P1/2.
    let (_dir2, rows2) = run_compare(&["--time-format", "%Y-%m-%dT%H"]);
    assert_eq!(get(&rows2[0], "matched_profiles"), "1");
    assert_eq!(get(&rows2[0], "same_nobs"), "0");
    assert_eq!(get(&rows2[0], "diff_nobs"), "1");
}

#[test]
fn rounding_decimals_control_position_matching() {
    // A P1/1 (10.0001/60.0001) and B P1/1 (10.0004/59.9996) agree to 3 decimals.
    let (_dir, rows) = run_compare(&[]);
    assert_eq!(get(&rows[0], "matched_profiles"), "2");

    // At 4 decimals they differ, so only the exactly equal P1/2 pair matches.
    let (_dir2, rows2) = run_compare(&["--decimals", "4"]);
    assert_eq!(get(&rows2[0], "matched_profiles"), "1");
}

#[test]
fn no_platform_key_matches_across_platforms() {
    // A P2/1 and B P4/1 share a time and position but not a platform code.
    let (_dir, rows) = run_compare(&["--no-platform-key"]);

    let b = &rows[0];
    assert_eq!(get(b, "matched_profiles"), "3");
    assert_eq!(get(b, "profile_cov_pct"), "100");
    assert_eq!(get(b, "same_nobs"), "2"); // P1/1 and P4/1
    assert_eq!(get(b, "diff_nobs"), "1"); // P1/2
    assert_eq!(get(b, "nobs_agree_pct"), "66.67");

    let a = &rows[1];
    assert_eq!(get(a, "matched_profiles"), "3");
    assert_eq!(get(a, "profile_cov_pct"), "75"); // 3 of 4, one is unkeyed

    // Platform counts describe the files themselves, so they are unaffected.
    assert_eq!(get(b, "common_platforms"), "1");
}

#[test]
fn the_datetime_column_gives_the_same_answer_as_days_since_1950() {
    let (_dir, by_days) = run_compare(&[]);
    let (_dir2, by_datetime) = run_compare(&["--time-col", "profile_timestamp"]);
    for (d, t) in by_days.iter().zip(by_datetime.iter()) {
        assert_eq!(
            get(d, "matched_profiles"),
            get(t, "matched_profiles"),
            "profile_time and profile_timestamp describe the same instants"
        );
        assert_eq!(get(d, "profile_cov_pct"), get(t, "profile_cov_pct"));
        assert_eq!(get(d, "same_nobs"), get(t, "same_nobs"));
    }
}

#[test]
fn identical_files_cover_each_other_completely() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("one.parquet");
    let b = dir.path().join("two.parquet");
    let out = dir.path().join("cmp.tsv");
    // No unkeyed profile here, so coverage can reach 100%.
    let profiles: Vec<Profile> = profiles_a().into_iter().filter(|p| p.0 != "P3").collect();
    write_fixture(&a, &profiles);
    write_fixture(&b, &profiles);

    dispatch(&[
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        out.to_str().unwrap(),
    ]);

    for row in read_tsv(&out) {
        assert_eq!(get(&row, "platform_cov_pct"), "100");
        assert_eq!(get(&row, "profile_cov_pct"), "100");
        assert_eq!(get(&row, "nobs_agree_pct"), "100");
        assert_eq!(get(&row, "diff_nobs"), "0");
    }
}

#[test]
fn disjoint_files_share_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("one.parquet");
    let b = dir.path().join("two.parquet");
    let out = dir.path().join("cmp.tsv");
    write_fixture(&a, &[("X", 1, 2, DAY_2020_01_01, 1.0, 1.0)]);
    write_fixture(&b, &[("Y", 1, 2, DAY_2020_06_15, 50.0, 50.0)]);

    dispatch(&[
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        out.to_str().unwrap(),
    ]);

    for row in read_tsv(&out) {
        assert_eq!(get(&row, "common_platforms"), "0");
        assert_eq!(get(&row, "matched_profiles"), "0");
        assert_eq!(get(&row, "profile_cov_pct"), "0");
        // No matched profiles means the agreement percentage has no denominator.
        assert_eq!(get(&row, "nobs_agree_pct"), "");
    }
}

#[test]
fn a_missing_key_column_is_a_clean_error() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("one.parquet");
    let b = dir.path().join("two.parquet");
    write_fixture(&a, &profiles_a());
    write_fixture(&b, &profiles_b());

    let args: Vec<String> = ["compare", "--lon-col", "lon", a.to_str().unwrap(), b.to_str().unwrap()]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let err = handle_dispatch(&args).expect_err("a misspelled column should fail");
    let msg = err.to_string();
    assert!(msg.contains("lon"), "error should name the missing column: {msg}");
    assert!(msg.contains("longitude"), "error should list what is available: {msg}");
}

#[test]
fn json_and_text_formats_render() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("one.parquet");
    let b = dir.path().join("two.parquet");
    write_fixture(&a, &profiles_a());
    write_fixture(&b, &profiles_b());

    let json = dir.path().join("cmp.json");
    dispatch(&[
        "compare",
        "--format",
        "json",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        json.to_str().unwrap(),
    ]);
    let text = fs::read_to_string(&json).unwrap();
    assert!(text.starts_with('['), "JSON output should be an array: {text}");
    assert!(text.contains("\"matched_profiles\""));

    let plain = dir.path().join("cmp.txt");
    dispatch(&[
        "compare",
        "--format",
        "text",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        plain.to_str().unwrap(),
    ]);
    let rendered = fs::read_to_string(&plain).unwrap();
    assert!(rendered.contains("reference"));
    assert!(rendered.contains("matched_profiles"));
}
