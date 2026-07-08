//! Parallel-renumbering regression test for `concat convert --threads`.
//!
//! With `--threads N` (N > 1) each platform range is renumbered on its own thread
//! into a temporary Parquet file beside the output, then the temp files are
//! concatenated in range order. Because ranges own complete platforms, the result
//! must be identical to the sequential path — only the process differs.
//!
//! This test forces `CTDDUMP_CHUNK_ROWS=1` so the two AR platforms land in separate
//! ranges (hence separate threads and temp files), exercising the parallel assembly
//! and the ordered temp-file merge. It is its own test binary (single test) so the
//! env var cannot race another test.

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

/// Run `concat convert` into a fresh output dir, returning (result DataFrame,
/// number of files left in the output dir).
fn concat(parquet_dir: &Path, extra: &[&str]) -> (DataFrame, usize) {
    let out_dir = tempfile::tempdir().unwrap();
    let out = out_dir.path().join("merged.parquet");
    let mut args = vec!["concat".to_string(), "convert".to_string()];
    args.extend(extra.iter().map(|s| s.to_string()));
    args.push(parquet_dir.to_str().unwrap().to_string());
    args.push(out.to_str().unwrap().to_string());
    handle_dispatch(&args).expect("concat convert should succeed");

    let df = ParquetReader::new(fs::File::open(&out).unwrap()).finish().unwrap();
    let file_count = fs::read_dir(out_dir.path()).unwrap().count();
    // Keep the dir alive until after we've counted its entries.
    drop(out_dir);
    (df, file_count)
}

#[test]
fn test_concat_convert_threads_matches_sequential() {
    std::env::set_var("CTDDUMP_CHUNK_ROWS", "1"); // one platform per range

    let parquet_dir = make_parquet_dir(
        &["./tests/test_data/AR_PR_CT_ITP-71.nc", "./tests/test_data/AR_PR_CT_58KN.nc"],
        "nrt_ar",
    );

    let (seq, _) = concat(parquet_dir.path(), &["--threads", "1"]);
    let (par, par_files) = concat(parquet_dir.path(), &["--threads", "4"]);

    // No temporary files left behind — only the final output remains.
    assert_eq!(par_files, 1, "temp files should be cleaned up, leaving only the output");

    assert_eq!(seq.height(), par.height(), "row count must match sequential");
    for name in ["platform_code", "profile_no", "observation_no", "profile_timestamp", "pres"] {
        let a = seq.column(name).unwrap();
        let b = par.column(name).unwrap();
        assert!(
            a.equals(b),
            "column `{name}` differs between sequential and --threads output",
        );
    }
}
