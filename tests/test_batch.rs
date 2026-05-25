use std::fs;
use ctddump::handle_dispatch;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Copy `src` files into a fresh temp dir and return (temp_dir, src_path).
fn setup_src(files: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    for f in files {
        fs::copy(f, dir.path().join(std::path::Path::new(f).file_name().unwrap())).unwrap();
    }
    let path = dir.path().to_path_buf();
    (dir, path)
}

// ── batch convert nrt_ar ──────────────────────────────────────────────────────

#[test]
fn test_batch_convert_nrt_ar_with_output_dir() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
        "./tests/test_data/AR_PR_CT_58KN.nc",
    ]);
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_ar".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    assert!(out_dir.path().join("AR_PR_CT_ITP-71.parquet").exists());
    assert!(out_dir.path().join("AR_PR_CT_58KN.parquet").exists());
}

#[test]
fn test_batch_convert_nrt_ar_in_place() {
    let (src_guard, src_dir) = setup_src(&[
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
    ]);

    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_ar".to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    assert!(src_guard.path().join("AR_PR_CT_ITP-71.parquet").exists());
}

#[test]
fn test_batch_convert_nrt_ar_with_threads() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
        "./tests/test_data/AR_PR_CT_58KN.nc",
    ]);
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_ar".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        "--threads".to_string(), "2".to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    assert!(out_dir.path().join("AR_PR_CT_ITP-71.parquet").exists());
    assert!(out_dir.path().join("AR_PR_CT_58KN.parquet").exists());
}

// ── batch convert cora ────────────────────────────────────────────────────────

#[test]
fn test_batch_convert_cora_with_output_dir() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc",
    ]);
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "convert".to_string(), "cora".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    assert!(out_dir.path().join("CO_DMQCGL01_19861204_PR_CT.parquet").exists());
}

// ── batch header nrt ──────────────────────────────────────────────────────────

#[test]
fn test_batch_header_nrt_with_output_dir() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
        "./tests/test_data/AR_PR_CT_58KN.nc",
    ]);
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "header".to_string(), "nrt".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    assert!(out_dir.path().join("AR_PR_CT_ITP-71.yaml").exists());
    assert!(out_dir.path().join("AR_PR_CT_58KN.yaml").exists());
}

#[test]
fn test_batch_header_nrt_in_place() {
    let (src_guard, src_dir) = setup_src(&[
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
    ]);

    let args = vec![
        "batch".to_string(), "header".to_string(), "nrt".to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    assert!(src_guard.path().join("AR_PR_CT_ITP-71.yaml").exists());
}

// ── batch header cora ─────────────────────────────────────────────────────────

#[test]
fn test_batch_header_cora_with_output_dir() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc",
    ]);
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "header".to_string(), "cora".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    assert!(out_dir.path().join("CO_DMQCGL01_19861204_PR_CT.yaml").exists());
}

// ── duplicate detection ───────────────────────────────────────────────────────

#[test]
fn test_batch_duplicate_error() {
    let src_root = tempfile::tempdir().unwrap();
    let sub_a = src_root.path().join("a");
    let sub_b = src_root.path().join("b");
    fs::create_dir_all(&sub_a).unwrap();
    fs::create_dir_all(&sub_b).unwrap();

    fs::copy(
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
        sub_a.join("AR_PR_CT_ITP-71.nc"),
    ).unwrap();
    fs::copy(
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
        sub_b.join("AR_PR_CT_ITP-71.nc"),
    ).unwrap();

    let out_dir = tempfile::tempdir().unwrap();
    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_ar".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_root.path().to_str().unwrap().to_string(),
    ];

    let result = handle_dispatch(&args);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("Duplicate output filenames detected"), "got: {msg}");
}

// ── empty directory ───────────────────────────────────────────────────────────

#[test]
fn test_batch_empty_dir_error() {
    let src_dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_ar".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.path().to_str().unwrap().to_string(),
    ];

    let result = handle_dispatch(&args);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("No .nc files found"), "got: {msg}");
}
