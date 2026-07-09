//! Streaming-conversion regression tests.
//!
//! The converters process each NetCDF file in `TIME` / `N_PROF` chunks and write
//! one Parquet row group per chunk (see `common::time_chunks`). These tests force
//! the smallest possible chunk (one outer step per chunk) via `CTDDUMP_CHUNK_ROWS`
//! so that every chunk boundary is exercised, then confirm the conversion still
//! succeeds and produces a non-empty Parquet file.
//!
//! This guards the failure modes specific to chunking — rank-0 scalar reads,
//! zero-row schema chunks, and cross-chunk profile/deployment numbering — which
//! the whole-file path never hit. Value-level equivalence to the non-streamed
//! output is verified out-of-band against reference Parquet files.
//!
//! This file is its own test binary, so setting `CTDDUMP_CHUNK_ROWS` here does not
//! leak into the other integration tests (which run in separate processes).

use std::path::PathBuf;

use ctddump::handle_dispatch;

/// Convert `src` as `format` with one outer step per chunk and assert the output
/// Parquet file exists and is non-empty.
fn convert_streamed(format: &str, src: &str) {
    // Smallest chunk → maximum number of chunk boundaries.
    std::env::set_var("CTDDUMP_CHUNK_ROWS", "1");

    let dest = std::env::temp_dir().join(format!(
        "ctddump_stream_{}_{}.parquet",
        format,
        PathBuf::from(src).file_stem().unwrap().to_string_lossy()
    ));
    let dest_str = dest.to_string_lossy().into_owned();

    let args = vec![
        "convert".to_string(),
        format.to_string(),
        src.to_string(),
        dest_str.clone(),
    ];
    let result = handle_dispatch(&args);
    assert!(result.is_ok(), "streamed convert failed for {src}: {result:?}");

    let size = std::fs::metadata(&dest)
        .unwrap_or_else(|e| panic!("output missing for {src}: {e}"))
        .len();
    assert!(size > 0, "streamed output is empty for {src}");

    let _ = std::fs::remove_file(&dest);
}

#[test]
fn test_stream_nrt_ar() {
    convert_streamed("nrt_ar", "./tests/test_data/AR_PR_CT_ITP-71.nc");
}

/// BO_PR_CT_KBH1723 uses DEPLOY_* coordinates expanded via the DEPLOYMENT index,
/// which resolves against absolute TIME positions — the case most sensitive to
/// chunk boundaries.
#[test]
fn test_stream_nrt_bo_deploy() {
    convert_streamed("nrt_bo", "./tests/test_data/BO_PR_CT_KBH1723.nc");
}

#[test]
fn test_stream_cora() {
    convert_streamed("cora", "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc");
}

#[test]
fn test_stream_cora_legacy() {
    convert_streamed("cora_legacy", "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc");
}
