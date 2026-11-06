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

/// A pattern that matches no files is not an error: the batch succeeds and writes
/// no output (so a workflow can reference a not-yet-available dataset).
#[test]
fn test_batch_empty_pattern_no_output() {
    let src_dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_ar".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.path().to_str().unwrap().to_string(),
    ];

    let result = handle_dispatch(&args);
    assert!(result.is_ok(), "empty match should succeed, got: {result:?}");
    let produced = std::fs::read_dir(out_dir.path()).unwrap().count();
    assert_eq!(produced, 0, "no output files should be written");
}

// ── pattern selection ─────────────────────────────────────────────────────────

/// Default pattern for nrt_ar is "AR_PR_CT_*.nc"; a CORA file in the same
/// directory must not be converted.
#[test]
fn test_batch_default_pattern_filters() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
        "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc",
    ]);
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_ar".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    assert!(out_dir.path().join("AR_PR_CT_ITP-71.parquet").exists());
    assert!(!out_dir.path().join("CO_DMQCGL01_19861204_PR_CT.parquet").exists());
}

/// --pattern overrides the default; here we narrow two AR files down to one
/// using an exact filename.
#[test]
fn test_batch_pattern_override() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
        "./tests/test_data/AR_PR_CT_58KN.nc",
    ]);
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_ar".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        "--pattern".to_string(), "AR_PR_CT_ITP-71.nc".to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    assert!(handle_dispatch(&args).is_ok());

    assert!(out_dir.path().join("AR_PR_CT_ITP-71.parquet").exists());
    assert!(!out_dir.path().join("AR_PR_CT_58KN.parquet").exists());
}

// ── --config files for NRT and CORA ───────────────────────────────────────────

/// Write `content` to a TOML file in `dir` and return its path as a string.
fn write_config(dir: &std::path::Path, content: &str) -> String {
    let path = dir.join("config.toml");
    fs::write(&path, content).unwrap();
    path.to_str().unwrap().to_string()
}

/// Names of the files in `dir`, sorted.
fn file_names(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn test_batch_convert_nrt_config_file_and_cli_override() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/BO_PR_CT_ARH160003.nc",
        "./tests/test_data/BO_PR_CT_SMHI3125.nc",
        "./tests/test_data/BO_PR_CT_SMHIHAVSTENSFJORD.nc",
    ]);
    let cfg_dir = tempfile::tempdir().unwrap();
    let config = write_config(
        cfg_dir.path(),
        "has_deph_source = true\nhas_profile_coords = true\npattern = \"BO_PR_CT_SMHI*.nc\"\n",
    );

    // The pattern comes from the config file.
    let out_dir = tempfile::tempdir().unwrap();
    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_bo".to_string(),
        "--config".to_string(), config.clone(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    handle_dispatch(&args).unwrap();
    assert_eq!(
        file_names(out_dir.path()),
        vec!["BO_PR_CT_SMHI3125.parquet", "BO_PR_CT_SMHIHAVSTENSFJORD.parquet"]
    );

    // A command-line flag overrides the config file.
    let out_dir = tempfile::tempdir().unwrap();
    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_bo".to_string(),
        "--config".to_string(), config,
        "--pattern".to_string(), "BO_PR_CT_ARH*.nc".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    handle_dispatch(&args).unwrap();
    assert_eq!(file_names(out_dir.path()), vec!["BO_PR_CT_ARH160003.parquet"]);
}

#[test]
fn test_batch_convert_cora_config_file() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc",
        "./tests/test_data/CO_DMQCGL01_20201005_PR_CT.nc",
        "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc",
    ]);
    let batch = |config: Option<&str>, out: &std::path::Path| {
        let mut args = vec!["batch".to_string(), "convert".to_string(), "cora_legacy".to_string()];
        match config {
            Some(c) => args.extend(["--config".to_string(), c.to_string()]),
            None => args.extend(["--pattern".to_string(), "CO_*_1986*.nc".to_string()]),
        }
        args.extend([
            "--output".to_string(), out.to_str().unwrap().to_string(),
            src_dir.to_str().unwrap().to_string(),
        ]);
        handle_dispatch(&args)
    };

    // The 1986 file is in the current CORA layout, so the legacy defaults
    // cannot read it.
    let out_dir = tempfile::tempdir().unwrap();
    assert!(batch(None, out_dir.path()).is_err());

    // A config file describing the current layout replaces every legacy
    // default, and its pattern selects only the 1986 file.
    let cfg_dir = tempfile::tempdir().unwrap();
    let config = write_config(
        cfg_dir.path(),
        "time_var = \"TIME\"\nqc_type = \"int\"\nhas_time_qc = true\nhas_deph_source = true\n\
         pattern = \"CO_*_1986*.nc\"\n",
    );
    let out_dir = tempfile::tempdir().unwrap();
    batch(Some(&config), out_dir.path()).unwrap();
    assert_eq!(file_names(out_dir.path()), vec!["CO_DMQCGL01_19861204_PR_CT.parquet"]);
}

#[test]
fn test_convert_invalid_config_file_errors() {
    let cfg_dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let dest = out_dir.path().join("out.parquet").to_str().unwrap().to_string();

    // A required field is missing.
    let config = write_config(cfg_dir.path(), "has_deph_source = true\n");
    let args = vec![
        "convert".to_string(), "nrt_ar".to_string(),
        "--config".to_string(), config,
        "./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(), dest.clone(),
    ];
    let err = handle_dispatch(&args).unwrap_err().to_string();
    assert!(err.contains("Invalid config file"), "unexpected error: {err}");

    // The file does not exist.
    let missing = cfg_dir.path().join("missing.toml").to_str().unwrap().to_string();
    let args = vec![
        "convert".to_string(), "cora".to_string(),
        "--config".to_string(), missing,
        "./tests/test_data/CO_DMQCGL01_20201005_PR_CT.nc".to_string(), dest,
    ];
    let err = handle_dispatch(&args).unwrap_err().to_string();
    assert!(err.contains("Cannot read config file"), "unexpected error: {err}");
    assert!(!out_dir.path().join("out.parquet").exists());
}

// ── failures ──────────────────────────────────────────────────────────────────

#[test]
fn test_batch_convert_reports_failed_files_and_converts_the_rest() {
    let (_src_guard, src_dir) = setup_src(&["./tests/test_data/AR_PR_CT_ITP-71.nc"]);
    fs::write(src_dir.join("AR_PR_CT_BROKEN.nc"), "not a NetCDF file").unwrap();
    let out_dir = tempfile::tempdir().unwrap();

    let args = vec![
        "batch".to_string(), "convert".to_string(), "nrt_ar".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    let err = handle_dispatch(&args).unwrap_err().to_string();
    assert!(err.starts_with("Batch processing errors:"), "unexpected error: {err}");
    assert!(err.contains("AR_PR_CT_BROKEN.nc"), "error should name the bad file: {err}");
    assert!(!err.contains("ITP-71"), "the good file should not be reported: {err}");
    assert!(out_dir.path().join("AR_PR_CT_ITP-71.parquet").exists(), "good file still converted");
}

#[test]
fn test_single_file_failures_return_errors() {
    // `convert` and `header` return an error for an unreadable file (so the
    // caller decides what to do) rather than exiting the process.
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("broken.nc");
    fs::write(&bad, "not a NetCDF file").unwrap();
    let bad = bad.to_str().unwrap().to_string();
    let dest = |ext: &str| dir.path().join(format!("out.{ext}")).to_str().unwrap().to_string();

    for args in [
        vec!["convert", "nrt_ar"],
        vec!["convert", "cora"],
        vec!["header", "nrt"],
        vec!["header", "cora"],
    ] {
        let ext = if args[0] == "convert" { "parquet" } else { "yaml" };
        let mut args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        args.extend([bad.clone(), dest(ext)]);
        assert!(handle_dispatch(&args).is_err(), "{args:?} should fail");
    }
}

// ── default patterns per region ───────────────────────────────────────────────

#[test]
fn test_batch_convert_region_default_patterns() {
    // One file per NRT region in the same directory: each region's batch picks
    // only its own files through its built-in pattern.
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
        "./tests/test_data/BO_PR_CT_SMHI3125.nc",
        "./tests/test_data/MO_PR_CT_SicilyChannel_2017.nc",
        "./tests/test_data/GL_PR_CT_EXEC004K.nc",
    ]);
    for (region, expect) in [
        ("nrt_ar", "AR_PR_CT_ITP-71.parquet"),
        ("nrt_bo", "BO_PR_CT_SMHI3125.parquet"),
        ("nrt_mo", "MO_PR_CT_SicilyChannel_2017.parquet"),
        ("nrt_gl", "GL_PR_CT_EXEC004K.parquet"),
    ] {
        let out_dir = tempfile::tempdir().unwrap();
        let args = vec![
            "batch".to_string(), "convert".to_string(), region.to_string(),
            "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
            src_dir.to_str().unwrap().to_string(),
        ];
        handle_dispatch(&args).unwrap();
        assert_eq!(file_names(out_dir.path()), vec![expect], "{region}");
    }
}

#[test]
fn test_batch_header_generic() {
    let (_src_guard, src_dir) = setup_src(&[
        "./tests/test_data/AR_PR_CT_ITP-71.nc",
        "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc",
    ]);
    let out_dir = tempfile::tempdir().unwrap();
    let args = vec![
        "batch".to_string(), "header".to_string(), "generic".to_string(),
        "--output".to_string(), out_dir.path().to_str().unwrap().to_string(),
        src_dir.to_str().unwrap().to_string(),
    ];
    handle_dispatch(&args).unwrap();
    assert_eq!(
        file_names(out_dir.path()),
        vec!["AR_PR_CT_ITP-71.yaml", "CO_DMQCGL01_19861204_PR_CT.yaml"]
    );
}
