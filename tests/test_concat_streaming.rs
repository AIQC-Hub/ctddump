//! Chunk-size-independence regression test for `concat convert`.
//!
//! `concat convert` bounds memory by merging one contiguous `platform_code` range
//! at a time and writing one Parquet row group per range (range size is capped by
//! `CTDDUMP_CHUNK_ROWS`). Because `renumber` partitions by `platform_code`, the
//! merged data must be identical no matter how the platforms are split into ranges —
//! only the on-disk row-group layout may change.
//!
//! This test merges the same inputs twice, once with a large budget (a single range,
//! one row group) and once with `CTDDUMP_CHUNK_ROWS=1` (each platform its own range),
//! and asserts the renumbering and row order are identical.
//!
//! This file is its own test binary, so mutating `CTDDUMP_CHUNK_ROWS` here does not
//! leak into the other integration tests. It contains a single test, so the two
//! sequential env changes below cannot race another test in this process.

use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

fn setup_src(files: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    for f in files {
        fs::copy(f, dir.path().join(Path::new(f).file_name().unwrap())).unwrap();
    }
    let path = dir.path().to_path_buf();
    (dir, path)
}

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

fn concat_to(parquet_dir: &Path, chunk_rows: &str) -> DataFrame {
    std::env::set_var("CTDDUMP_CHUNK_ROWS", chunk_rows);
    let output = tempfile::NamedTempFile::with_suffix(".parquet").unwrap();
    let args = vec![
        "concat".to_string(), "convert".to_string(),
        parquet_dir.to_str().unwrap().to_string(),
        output.path().to_str().unwrap().to_string(),
    ];
    handle_dispatch(&args).expect("concat convert should succeed");
    let f = fs::File::open(output.path()).unwrap();
    ParquetReader::new(f).finish().unwrap()
}

#[test]
fn test_concat_convert_is_chunk_size_independent() {
    let parquet_dir = make_parquet_dir(
        &["./tests/test_data/AR_PR_CT_ITP-71.nc", "./tests/test_data/AR_PR_CT_58KN.nc"],
        "nrt_ar",
    );

    // One range (all platforms together) vs. one range per platform.
    let big = concat_to(parquet_dir.path(), "100000000");
    let small = concat_to(parquet_dir.path(), "1");

    assert_eq!(big.height(), small.height(), "row count must not depend on chunk size");

    // The renumbering and row order (the only things a range split could disturb)
    // must be byte-identical. These columns are never NaN, so direct Series equality
    // is reliable.
    for name in ["platform_code", "profile_no", "observation_no", "profile_timestamp"] {
        let a = big.column(name).unwrap();
        let b = small.column(name).unwrap();
        assert!(
            a.equals(b),
            "column `{name}` differs between chunk sizes — renumbering is not chunk-independent",
        );
    }
}
