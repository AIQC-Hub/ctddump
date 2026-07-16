//! Integration tests for the `report` subcommand. Fixtures are built in-test
//! (no external test data required).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("report should succeed");
}

/// Parse a TSV report into (header, rows).
fn parse_tsv(path: &Path) -> (Vec<String>, Vec<Vec<String>>) {
    let text = fs::read_to_string(path).unwrap();
    let mut lines = text.lines();
    let header: Vec<String> = lines.next().unwrap().split('\t').map(str::to_string).collect();
    let rows: Vec<Vec<String>> =
        lines.map(|l| l.split('\t').map(str::to_string).collect()).collect();
    (header, rows)
}

fn as_map<'a>(header: &'a [String], row: &'a [String]) -> HashMap<&'a str, &'a str> {
    header.iter().map(String::as_str).zip(row.iter().map(String::as_str)).collect()
}

/// Small ctddump-shaped Parquet fixture:
///   platform A: profile 1 (temp 1,2,NaN), profile 2 (temp 3,4); all qc "1"
///   platform B: profile 1 (temp NaN); time_qc "9"
fn write_parquet_fixture(path: &Path) {
    let mut df = DataFrame::new(vec![
        Series::new("platform_code".into(), vec!["A", "A", "A", "A", "A", "B"]),
        Series::new("profile_no".into(), vec![1u32, 1, 1, 2, 2, 1]),
        Series::new("temp".into(), vec![1.0f32, 2.0, f32::NAN, 3.0, 4.0, f32::NAN]),
        Series::new("psal".into(), vec![10.0f32, 11.0, 12.0, 13.0, 14.0, f32::NAN]),
        Series::new("pres".into(), vec![0.0f32, 5.0, 10.0, 0.0, 5.0, 100.0]),
        Series::new("time_qc".into(), vec!["1", "1", "1", "1", "1", "9"]),
        Series::new("position_qc".into(), vec!["1", "1", "1", "1", "1", "1"]),
        // A spans lon 1..5 / lat 20..24; B is at lon 10 / lat 30.
        Series::new("longitude".into(), vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 10.0]),
        Series::new("latitude".into(), vec![20.0f32, 21.0, 22.0, 23.0, 24.0, 30.0]),
        Series::new("profile_timestamp".into(), vec![0i64, 0, 0, 0, 0, 1000])
            .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
            .unwrap(),
    ])
    .unwrap();
    let f = fs::File::create(path).unwrap();
    ParquetWriter::new(f).finish(&mut df).unwrap();
}

#[test]
fn test_report_parquet_platform() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("data.parquet");
    let out = dir.path().join("report.tsv");
    write_parquet_fixture(&src);

    dispatch(&[
        "report", "parquet", "--level", "platform",
        src.to_str().unwrap(), out.to_str().unwrap(),
    ]);

    let (header, rows) = parse_tsv(&out);
    assert_eq!(rows.len(), 2, "one row per platform");
    let by_platform: HashMap<&str, HashMap<&str, &str>> =
        rows.iter().map(|r| (r[0].as_str(), as_map(&header, r))).collect();

    let a = &by_platform["A"];
    assert_eq!(a["n_profiles"], "2");
    assert_eq!(a["n_obs"], "5");
    assert_eq!(a["time_qc_good"], "2");
    assert_eq!(a["position_qc_good"], "2");
    assert_eq!(a["na_temp"], "1");
    assert_eq!(a["temp_min"], "1");
    assert_eq!(a["temp_max"], "4");
    assert_eq!(a["temp_mean"], "2.5"); // (1+2+3+4)/4
    assert_eq!(a["longitude_min"], "1");
    assert_eq!(a["longitude_max"], "5");
    assert_eq!(a["latitude_min"], "20");
    assert_eq!(a["latitude_max"], "24");

    let b = &by_platform["B"];
    assert_eq!(b["n_profiles"], "1");
    assert_eq!(b["n_obs"], "1");
    assert_eq!(b["time_qc_good"], "0"); // flag is "9"
    assert_eq!(b["na_temp"], "1");
    assert_eq!(b["temp_min"], "", "all-NaN group → empty stat");
    assert_eq!(b["temp_mean"], "");
    assert_eq!(b["longitude_min"], "10");
    assert_eq!(b["latitude_max"], "30");
}

#[test]
fn test_report_parquet_global() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("data.parquet");
    let out = dir.path().join("report.tsv");
    write_parquet_fixture(&src);

    dispatch(&["report", "parquet", "--level", "global", src.to_str().unwrap(), out.to_str().unwrap()]);

    let (header, rows) = parse_tsv(&out);
    assert_eq!(rows.len(), 1);
    let g = as_map(&header, &rows[0]);
    assert_eq!(g["n_platforms"], "2");
    assert_eq!(g["n_profiles"], "3");
    assert_eq!(g["n_obs"], "6");
    assert_eq!(g["time_qc_good"], "2");
    assert_eq!(g["na_temp"], "2");
    assert_eq!(g["temp_min"], "1");
    assert_eq!(g["temp_max"], "4");
    assert_eq!(g["longitude_min"], "1");
    assert_eq!(g["longitude_max"], "10");
    assert_eq!(g["latitude_min"], "20");
    assert_eq!(g["latitude_max"], "30");
}

#[test]
fn test_report_parquet_profile() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("data.parquet");
    let out = dir.path().join("report.tsv");
    write_parquet_fixture(&src);

    dispatch(&["report", "parquet", "--level", "profile", src.to_str().unwrap(), out.to_str().unwrap()]);

    let (header, rows) = parse_tsv(&out);
    assert_eq!(rows.len(), 3, "one row per (platform, profile)");
    // Sorted by platform_code, profile_no: A/1, A/2, B/1
    let a1 = as_map(&header, &rows[0]);
    assert_eq!(a1["platform_code"], "A");
    assert_eq!(a1["profile_no"], "1");
    assert_eq!(a1["n_obs"], "3");
    assert_eq!(a1["time_qc"], "1");
    assert_eq!(a1["na_temp"], "1");
}

#[test]
fn test_report_parquet_json_is_valid() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("data.parquet");
    let out = dir.path().join("report.json");
    write_parquet_fixture(&src);

    dispatch(&["report", "parquet", "--level", "global", "--format", "json",
        src.to_str().unwrap(), out.to_str().unwrap()]);

    let text = fs::read_to_string(&out).unwrap();
    let v: serde_yaml::Value = serde_yaml::from_str(&text).unwrap(); // YAML is a JSON superset
    let arr = v.as_sequence().expect("JSON array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0].get("n_platforms").unwrap().as_i64(), Some(2));
    assert_eq!(arr[0].get("n_obs").unwrap().as_i64(), Some(6));
}

const YAML_FIXTURE: &str = r#"
FILE_A:
  variables:
    TEMP:
      data_type: Float(F32)
      dimensions: [TIME, DEPTH]
    PSAL:
      data_type: Float(F32)
      dimensions: [TIME, DEPTH]
    TEMP_QC:
      data_type: Int(I8)
      dimensions: [TIME, DEPTH]
FILE_B:
  variables:
    TEMP:
      data_type: Float(F32)
      dimensions: [TIME, DEPTH]
    DOXY:
      data_type: Float(F32)
      dimensions: [TIME, DEPTH]
    TUR3:
      data_type: Float(F32)
      dimensions: [TIME, DEPTH]
    DOXY_QC:
      data_type: Int(I8)
      dimensions: [TIME, DEPTH]
    TIME_QC:
      data_type: Int(I8)
      dimensions: [TIME]
    POSITION_QC:
      data_type: Int(I8)
      dimensions: [TIME]
"#;

#[test]
fn test_report_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("headers.yaml");
    let out = dir.path().join("report.tsv");
    fs::write(&src, YAML_FIXTURE).unwrap();

    dispatch(&["report", "yaml", src.to_str().unwrap(), out.to_str().unwrap()]);

    let (header, rows) = parse_tsv(&out);
    assert_eq!(rows.len(), 2);
    let by_file: HashMap<&str, HashMap<&str, &str>> =
        rows.iter().map(|r| (r[0].as_str(), as_map(&header, r))).collect();

    let a = &by_file["FILE_A"];
    assert_eq!(a["has_temp"], "true");
    assert_eq!(a["has_psal"], "true");
    assert_eq!(a["has_pres"], "false");
    // The two profile-level QC flags are reported as a pair: FILE_A has neither.
    assert_eq!(a["has_time_qc"], "false");
    assert_eq!(a["has_position_qc"], "false");
    assert_eq!(a["extra_params"], "", "TEMP/PSAL are core; TEMP_QC excluded");

    let b = &by_file["FILE_B"];
    assert_eq!(b["has_temp"], "true");
    assert_eq!(b["has_psal"], "false");
    assert_eq!(b["has_time_qc"], "true");
    assert_eq!(b["has_position_qc"], "true");
    assert_eq!(b["extra_params"], "DOXY;TUR3", "sorted, QC/core excluded");
}
