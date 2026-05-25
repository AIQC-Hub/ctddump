use std::fs;
use ctddump::handle_dispatch;
use polars::prelude::*;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Copy `src` files into a fresh temp dir and return (temp_dir, dir_path).
fn setup_src(files: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    for f in files {
        fs::copy(f, dir.path().join(std::path::Path::new(f).file_name().unwrap())).unwrap();
    }
    let path = dir.path().to_path_buf();
    (dir, path)
}

/// Convert a set of NC files to Parquet inside a temp dir and return the dir.
fn make_parquet_dir(nc_files: &[&str], subcommand: &str) -> tempfile::TempDir {
    let out_dir = tempfile::tempdir().unwrap();
    let (_src_guard, src_dir) = setup_src(nc_files);

    let args: Vec<String> = vec![
        "batch".to_string(), "convert".to_string(), subcommand.to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    handle_dispatch(&args).unwrap();
    out_dir
}

fn read_parquet(path: &std::path::Path) -> DataFrame {
    let f = std::fs::File::open(path).unwrap();
    ParquetReader::new(f).finish().unwrap()
}

// ── basic merge ───────────────────────────────────────────────────────────────

#[test]
fn test_concat_merges_files_into_one() {
    let parquet_dir = make_parquet_dir(
        &["./tests/test_data/AR_PR_CT_ITP-71.nc", "./tests/test_data/AR_PR_CT_58KN.nc"],
        "nrt_ar",
    );
    let output = tempfile::NamedTempFile::with_suffix(".parquet").unwrap();

    let args = vec![
        "concat".to_string(),
        parquet_dir.path().to_str().unwrap().to_string(),
        output.path().to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());
    assert!(output.path().exists());

    // Row count must equal the sum of the two individual files
    let combined = read_parquet(output.path());
    let itp71 = read_parquet(&parquet_dir.path().join("AR_PR_CT_ITP-71.parquet"));
    let kn58  = read_parquet(&parquet_dir.path().join("AR_PR_CT_58KN.parquet"));
    assert_eq!(combined.height(), itp71.height() + kn58.height());
}

// ── profile_no renumbering ────────────────────────────────────────────────────

#[test]
fn test_concat_renumber_profile_no_starts_at_one() {
    let parquet_dir = make_parquet_dir(
        &["./tests/test_data/AR_PR_CT_ITP-71.nc", "./tests/test_data/AR_PR_CT_58KN.nc"],
        "nrt_ar",
    );
    let output = tempfile::NamedTempFile::with_suffix(".parquet").unwrap();

    let args = vec![
        "concat".to_string(),
        parquet_dir.path().to_str().unwrap().to_string(),
        output.path().to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    let df = read_parquet(output.path());
    let profile_no = df.column("profile_no").unwrap();
    let min = profile_no.min::<u32>().unwrap().unwrap();
    assert_eq!(min, 1, "profile_no should start at 1");
}

#[test]
fn test_concat_renumber_observation_no_starts_at_one_per_profile() {
    let parquet_dir = make_parquet_dir(
        &["./tests/test_data/AR_PR_CT_ITP-71.nc"],
        "nrt_ar",
    );
    let output = tempfile::NamedTempFile::with_suffix(".parquet").unwrap();

    let args = vec![
        "concat".to_string(),
        parquet_dir.path().to_str().unwrap().to_string(),
        output.path().to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    let df = read_parquet(output.path());
    let obs_no = df.column("observation_no").unwrap();
    let min = obs_no.min::<u32>().unwrap().unwrap();
    assert_eq!(min, 1, "observation_no should start at 1");
}

// ── --no-renumber ─────────────────────────────────────────────────────────────

#[test]
fn test_concat_no_renumber_preserves_row_count() {
    let parquet_dir = make_parquet_dir(
        &["./tests/test_data/AR_PR_CT_ITP-71.nc", "./tests/test_data/AR_PR_CT_58KN.nc"],
        "nrt_ar",
    );
    let output = tempfile::NamedTempFile::with_suffix(".parquet").unwrap();

    let args = vec![
        "concat".to_string(),
        "--no-renumber".to_string(),
        parquet_dir.path().to_str().unwrap().to_string(),
        output.path().to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    let combined = read_parquet(output.path());
    let itp71 = read_parquet(&parquet_dir.path().join("AR_PR_CT_ITP-71.parquet"));
    let kn58  = read_parquet(&parquet_dir.path().join("AR_PR_CT_58KN.parquet"));
    assert_eq!(combined.height(), itp71.height() + kn58.height());
}

// ── --pattern ────────────────────────────────────────────────────────────────

#[test]
fn test_concat_pattern_selects_subset() {
    // Put two different parquet files in the dir; pattern selects only one
    let parquet_dir = make_parquet_dir(
        &["./tests/test_data/AR_PR_CT_ITP-71.nc", "./tests/test_data/AR_PR_CT_58KN.nc"],
        "nrt_ar",
    );
    let output = tempfile::NamedTempFile::with_suffix(".parquet").unwrap();

    let args = vec![
        "concat".to_string(),
        "--pattern".to_string(), "AR_PR_CT_ITP-71.parquet".to_string(),
        parquet_dir.path().to_str().unwrap().to_string(),
        output.path().to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    let combined = read_parquet(output.path());
    let itp71   = read_parquet(&parquet_dir.path().join("AR_PR_CT_ITP-71.parquet"));
    assert_eq!(combined.height(), itp71.height());
}

// ── error cases ───────────────────────────────────────────────────────────────

#[test]
fn test_concat_empty_dir_error() {
    let src_dir = tempfile::tempdir().unwrap();
    let output  = tempfile::NamedTempFile::with_suffix(".parquet").unwrap();

    let args = vec![
        "concat".to_string(),
        src_dir.path().to_str().unwrap().to_string(),
        output.path().to_str().unwrap().to_string(),
    ];
    let result = handle_dispatch(&args);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No files matching"));
}
