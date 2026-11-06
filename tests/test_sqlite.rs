//! Integration tests for the `sqlite` subcommand. Fixtures are built in-test
//! (no external test data required).

use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;
use rusqlite::Connection;

fn dispatch(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).map(|_| ())
}

/// Write the standard fixture: 3 profiles across 2 platforms, 2 observations
/// each. `institution` is constant within a profile (passthrough candidate);
/// `chla` varies per observation. Row A/1 obs 2 has a NaN temp; platform B has
/// empty `time_qc` (both should become SQL NULL).
fn write_fixture(path: &Path) {
    let ts = Series::new(
        "profile_timestamp".into(),
        vec![1000i64, 1000, 2000, 2000, 3000, 3000],
    )
    .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
    .unwrap();

    let mut df = DataFrame::new(vec![
        Series::new("platform_code".into(), vec!["A", "A", "A", "A", "B", "B"]),
        Series::new("profile_no".into(), vec![1u32, 1, 2, 2, 1, 1]),
        Series::new("observation_no".into(), vec![1u32, 2, 1, 2, 1, 2]),
        Series::new("profile_time".into(), vec![100.0f64, 100.0, 200.0, 200.0, 300.0, 300.0]),
        ts,
        Series::new("longitude".into(), vec![10.5f32, 10.5, 11.5, 11.5, 20.0, 20.0]),
        Series::new("latitude".into(), vec![60.0f32, 60.0, 61.0, 61.0, 70.0, 70.0]),
        Series::new("time_qc".into(), vec!["1", "1", "1", "1", "", ""]),
        Series::new("position_qc".into(), vec!["1", "1", "1", "1", "1", "1"]),
        Series::new("filename".into(), vec!["fa", "fa", "fa", "fa", "fb", "fb"]),
        Series::new("temp".into(), vec![4.0f32, f32::NAN, 5.0, 5.1, 6.0, 6.1]),
        Series::new("temp_qc".into(), vec!["1", "1", "1", "1", "1", "1"]),
        Series::new("temp_dm".into(), vec!["D", "D", "R", "R", "A", ""]),
        Series::new("psal".into(), vec![35.0f32, 35.1, 34.0, 34.1, 33.0, 33.1]),
        Series::new("psal_qc".into(), vec!["1", "1", "1", "1", "1", "1"]),
        Series::new("psal_dm".into(), vec!["D", "D", "R", "R", "A", "A"]),
        Series::new("pres".into(), vec![0.0f32, 10.0, 0.0, 10.0, 0.0, 10.0]),
        Series::new("pres_qc".into(), vec!["1", "1", "1", "1", "1", "1"]),
        Series::new("pres_dm".into(), vec!["D", "D", "R", "R", "A", "A"]),
        Series::new("pres_conv".into(), vec![0i8, 0, 0, 0, 1, 1]),
        Series::new("deph".into(), vec![0.0f32, 9.9, 0.0, 9.9, 0.0, 9.9]),
        Series::new("deph_qc".into(), vec!["1", "1", "1", "1", "1", "1"]),
        Series::new("deph_conv".into(), vec![1i8, 1, 1, 1, 0, 0]),
        Series::new("institution".into(), vec!["IMR", "IMR", "IMR", "IMR", "NOAA", "NOAA"]),
        Series::new("chla".into(), vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]),
    ])
    .unwrap();
    ParquetWriter::new(std::fs::File::create(path).unwrap())
        .finish(&mut df)
        .unwrap();
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn test_sqlite_basic_normalisation() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");
    write_fixture(&src);

    dispatch(&["sqlite", src.to_str().unwrap(), out.to_str().unwrap()]).unwrap();

    let conn = Connection::open(&out).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM platform"), 2);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM profile"), 3);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation"), 6);

    // FK joins resolve: platform A has 2 profiles and 4 observations.
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM profile p JOIN platform pl ON p.platform_id=pl.platform_id \
             WHERE pl.platform_code='A'"
        ),
        2
    );
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM observation o \
             JOIN profile p ON o.profile_id=p.profile_id \
             JOIN platform pl ON p.platform_id=pl.platform_id WHERE pl.platform_code='A'"
        ),
        4
    );

    // NaN temp becomes NULL (exactly one such observation).
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation WHERE temp IS NULL"), 1);

    // Empty QC string becomes NULL: platform B's profile has NULL time_qc.
    let tq: Option<String> = conn
        .query_row(
            "SELECT p.time_qc FROM profile p JOIN platform pl ON p.platform_id=pl.platform_id \
             WHERE pl.platform_code='B'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tq, None);

    // Datetime is stored as its millisecond integer.
    let ts: i64 = conn
        .query_row(
            "SELECT p.profile_timestamp FROM profile p \
             JOIN platform pl ON p.platform_id=pl.platform_id \
             WHERE pl.platform_code='B'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ts, 3000);

    // Standard columns land in the right table: no `temp` on profile, no
    // `longitude` on observation.
    assert!(conn
        .prepare("SELECT temp FROM profile")
        .err()
        .is_some());
    assert!(conn
        .prepare("SELECT longitude FROM observation")
        .err()
        .is_some());
}

/// The data-mode columns are exported as TEXT observation columns, with an empty
/// string (fill) mapped to NULL; QC flags are stored as text too.
#[test]
fn test_sqlite_data_mode_columns() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");
    write_fixture(&src);

    dispatch(&["sqlite", src.to_str().unwrap(), out.to_str().unwrap()]).unwrap();

    let conn = Connection::open(&out).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation WHERE temp_dm = 'D'"), 2);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation WHERE temp_dm = 'R'"), 2);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation WHERE temp_dm = 'A'"), 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation WHERE temp_dm IS NULL"), 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation WHERE pres_dm = 'A'"), 2);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation WHERE psal_dm = 'D'"), 2);

    // Declared and stored as text, not coerced to numbers.
    for col in ["temp_dm", "psal_dm", "pres_dm", "temp_qc", "psal_qc", "pres_qc", "deph_qc"] {
        let declared: String = conn
            .query_row(
                "SELECT type FROM pragma_table_info('observation') WHERE name = ?1",
                [col],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(declared, "TEXT", "{col} should be declared TEXT");
    }
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM observation WHERE typeof(temp_qc) <> 'text'"),
        0
    );

    // Data mode is observation-grain: not on the profile table.
    assert!(conn.prepare("SELECT temp_dm FROM profile").err().is_some());
}

#[test]
fn test_sqlite_default_dest_extension() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    write_fixture(&src);

    // No dest given: defaults to the source stem with a .sqlite extension.
    dispatch(&["sqlite", src.to_str().unwrap()]).unwrap();

    let expected = dir.path().join("in.sqlite");
    assert!(expected.exists());
    let conn = Connection::open(&expected).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation"), 6);
}

#[test]
fn test_sqlite_refuses_existing_without_force() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");
    write_fixture(&src);

    dispatch(&["sqlite", src.to_str().unwrap(), out.to_str().unwrap()]).unwrap();
    // Second run without --force must fail rather than clobber.
    assert!(dispatch(&["sqlite", src.to_str().unwrap(), out.to_str().unwrap()]).is_err());
    // With --force it succeeds and the result is valid again.
    dispatch(&["sqlite", "--force", src.to_str().unwrap(), out.to_str().unwrap()]).unwrap();
    let conn = Connection::open(&out).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation"), 6);
}

#[test]
fn test_sqlite_added_literal_columns() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");
    write_fixture(&src);

    dispatch(&[
        "sqlite",
        "--add",
        "profile.dataset=NRT",
        "--add",
        "observation.batch=7",
        "--add",
        "platform.source=copernicus",
        src.to_str().unwrap(),
        out.to_str().unwrap(),
    ])
    .unwrap();

    let conn = Connection::open(&out).unwrap();
    // Constant text on every profile row.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM profile WHERE dataset='NRT'"), 3);
    // Constant integer on every observation row.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation WHERE batch=7"), 6);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM platform WHERE source='copernicus'"), 2);
}

#[test]
fn test_sqlite_added_passthrough_columns() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");
    write_fixture(&src);

    dispatch(&[
        "sqlite",
        "--add-col",
        "profile.institution",
        "--add-col",
        "observation.chla",
        src.to_str().unwrap(),
        out.to_str().unwrap(),
    ])
    .unwrap();

    let conn = Connection::open(&out).unwrap();
    // Profile-grain passthrough: one value per profile.
    let inst: String = conn
        .query_row(
            "SELECT p.institution FROM profile p \
             JOIN platform pl ON p.platform_id=pl.platform_id WHERE pl.platform_code='B'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(inst, "NOAA");
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM profile WHERE institution='IMR'"), 2);
    // Observation-grain passthrough: distinct per row.
    assert_eq!(count(&conn, "SELECT COUNT(DISTINCT chla) FROM observation"), 6);
}

#[test]
fn test_sqlite_passthrough_not_constant_errors() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");

    // `institution` varies within profile A/1, so it cannot be a profile column.
    let mut df = DataFrame::new(vec![
        Series::new("platform_code".into(), vec!["A", "A"]),
        Series::new("profile_no".into(), vec![1u32, 1]),
        Series::new("observation_no".into(), vec![1u32, 2]),
        Series::new("institution".into(), vec!["IMR", "NOAA"]),
    ])
    .unwrap();
    ParquetWriter::new(std::fs::File::create(&src).unwrap())
        .finish(&mut df)
        .unwrap();

    let res = dispatch(&[
        "sqlite",
        "--add-col",
        "profile.institution",
        src.to_str().unwrap(),
        out.to_str().unwrap(),
    ]);
    assert!(res.is_err());
}

#[test]
fn test_sqlite_streaming_chunk_independent() {
    // A profile split across a chunk boundary must still produce one profile row
    // and merge its passthrough value correctly.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");
    write_fixture(&src);

    std::env::set_var("CTDDUMP_CHUNK_ROWS", "3");
    dispatch(&[
        "sqlite",
        "--add-col",
        "profile.institution",
        src.to_str().unwrap(),
        out.to_str().unwrap(),
    ])
    .unwrap();
    std::env::remove_var("CTDDUMP_CHUNK_ROWS");

    let conn = Connection::open(&out).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM profile"), 3);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM observation"), 6);
    // Profile A/2, which straddles the chunk boundary, keeps its institution.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM profile WHERE institution='IMR'"), 2);
}

#[test]
fn test_sqlite_rejects_bad_target_table() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");
    write_fixture(&src);

    // Unknown table name.
    assert!(dispatch(&[
        "sqlite",
        "--add",
        "bogus.col=1",
        src.to_str().unwrap(),
        out.to_str().unwrap(),
    ])
    .is_err());
    // Collision with a standard column.
    assert!(dispatch(&[
        "sqlite",
        "--add",
        "observation.temp=1",
        src.to_str().unwrap(),
        out.to_str().unwrap(),
    ])
    .is_err());
}

/// Rewrite the standard fixture with a different `institution` column.
fn write_fixture_with_institution(path: &Path, institution: [&str; 6]) {
    write_fixture(path);
    let mut df = ParquetReader::new(std::fs::File::open(path).unwrap()).finish().unwrap();
    df.with_column(Series::new("institution".into(), institution.to_vec())).unwrap();
    ParquetWriter::new(std::fs::File::create(path).unwrap())
        .finish(&mut df)
        .unwrap();
}

#[test]
fn test_sqlite_added_platform_passthrough_column() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");
    // Platform A's first profile has an empty (unknown) institution; it must
    // yield to the value from A's second profile rather than conflict with it.
    write_fixture_with_institution(&src, ["", "", "IMR", "IMR", "NOAA", "NOAA"]);

    dispatch(&[
        "sqlite",
        "--add-col",
        "platform.institution",
        src.to_str().unwrap(),
        out.to_str().unwrap(),
    ])
    .unwrap();

    let conn = Connection::open(&out).unwrap();
    let inst = |code: &str| -> String {
        conn.query_row(
            "SELECT institution FROM platform WHERE platform_code=?1",
            [code],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(inst("A"), "IMR");
    assert_eq!(inst("B"), "NOAA");
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM platform"), 2);
}

#[test]
fn test_sqlite_platform_passthrough_not_constant_errors() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.parquet");
    let out = dir.path().join("out.sqlite");
    // Constant within each profile, but platform A's two profiles disagree.
    write_fixture_with_institution(&src, ["IMR", "IMR", "SMHI", "SMHI", "NOAA", "NOAA"]);

    let err = dispatch(&[
        "sqlite",
        "--add-col",
        "platform.institution",
        src.to_str().unwrap(),
        out.to_str().unwrap(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("not constant across platform `A`"), "unexpected error: {err}");
}
