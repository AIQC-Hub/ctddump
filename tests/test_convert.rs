use ctddump::{handle_dispatch, Config};
use polars::prelude::*;

#[test]
fn test_convert_nrt_ar_1() {
    let args = vec!["convert".to_string(), "nrt_ar".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_ar".to_string(),
        args: vec!["./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_nrt_ar_2() {
    let args = vec!["convert".to_string(), "nrt_ar".to_string(), "./tests/test_data/AR_PR_CT_58KN.nc".to_string(), "./tests/test_data/AR_PR_CT_58KN.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_ar".to_string(),
        args: vec!["./tests/test_data/AR_PR_CT_58KN.nc".to_string(), "./tests/test_data/AR_PR_CT_58KN.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_nrt_bo_1() {
    let args = vec!["convert".to_string(), "nrt_bo".to_string(), "./tests/test_data/BO_PR_CT_ARH160003.nc".to_string(), "./tests/test_data/BO_PR_CT_ARH160003.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_bo".to_string(),
        args: vec!["./tests/test_data/BO_PR_CT_ARH160003.nc".to_string(), "./tests/test_data/BO_PR_CT_ARH160003.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_nrt_bo_2() {
    let args = vec!["convert".to_string(), "nrt_bo".to_string(), "./tests/test_data/BO_PR_CT_BRK5059505.nc".to_string(), "./tests/test_data/BO_PR_CT_BRK5059505.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_bo".to_string(),
        args: vec!["./tests/test_data/BO_PR_CT_BRK5059505.nc".to_string(), "./tests/test_data/BO_PR_CT_BRK5059505.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_nrt_bo_3() {
    let args = vec!["convert".to_string(), "nrt_bo".to_string(), "./tests/test_data/BO_PR_CT_SMHIHAVSTENSFJORD.nc".to_string(), "./tests/test_data/BO_PR_CT_SMHIHAVSTENSFJORD.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_bo".to_string(),
        args: vec!["./tests/test_data/BO_PR_CT_SMHIHAVSTENSFJORD.nc".to_string(), "./tests/test_data/BO_PR_CT_SMHIHAVSTENSFJORD.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_nrt_bo_4() {
    let args = vec!["convert".to_string(), "nrt_bo".to_string(), "./tests/test_data/BO_PR_CT_SMHI3125.nc".to_string(), "./tests/test_data/BO_PR_CT_SMHI3125.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_bo".to_string(),
        args: vec!["./tests/test_data/BO_PR_CT_SMHI3125.nc".to_string(), "./tests/test_data/BO_PR_CT_SMHI3125.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

/// BO_PR_CT_KBH1723.nc has DEPLOY_LONGITUDE/DEPLOY_LATITUDE (no PRECISE_*)
/// so this exercises the deployment-index expansion path.
#[test]
fn test_convert_nrt_bo_5() {
    let args = vec!["convert".to_string(), "nrt_bo".to_string(), "./tests/test_data/BO_PR_CT_KBH1723.nc".to_string(), "./tests/test_data/BO_PR_CT_KBH1723.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_bo".to_string(),
        args: vec!["./tests/test_data/BO_PR_CT_KBH1723.nc".to_string(), "./tests/test_data/BO_PR_CT_KBH1723.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_nrt_mo_1() {
    let args = vec!["convert".to_string(), "nrt_mo".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_1990.nc".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_1990.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_mo".to_string(),
        args: vec!["./tests/test_data/MO_PR_CT_SicilyChannel_1990.nc".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_1990.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_nrt_mo_2() {
    let args = vec!["convert".to_string(), "nrt_mo".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_2017.nc".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_2017.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_mo".to_string(),
        args: vec!["./tests/test_data/MO_PR_CT_SicilyChannel_2017.nc".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_2017.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_nrt_mo_3() {
    let args = vec!["convert".to_string(), "nrt_mo".to_string(), "./tests/test_data/MO_PR_CT_SardiniaChannel_2008.nc".to_string(), "./tests/test_data/MO_PR_CT_SardiniaChannel_2008.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_mo".to_string(),
        args: vec!["./tests/test_data/MO_PR_CT_SardiniaChannel_2008.nc".to_string(), "./tests/test_data/MO_PR_CT_SardiniaChannel_2008.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_nrt_gl_1() {
    let args = vec!["convert".to_string(), "nrt_gl".to_string(), "./tests/test_data/GL_PR_CT_EXEC004K.nc".to_string(), "./tests/test_data/GL_PR_CT_EXEC004K.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_gl".to_string(),
        args: vec!["./tests/test_data/GL_PR_CT_EXEC004K.nc".to_string(), "./tests/test_data/GL_PR_CT_EXEC004K.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_cora_legacy_1() {
    let args = vec!["convert".to_string(), "cora_legacy".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "cora_legacy".to_string(),
        args: vec!["./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_cora_legacy_2() {
    let args = vec!["convert".to_string(), "cora_legacy".to_string(), "./tests/test_data/CO_DMQCGL01_20201005_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201005_PR_CT.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "cora_legacy".to_string(),
        args: vec!["./tests/test_data/CO_DMQCGL01_20201005_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201005_PR_CT.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_convert_cora_1() {
    let args = vec!["convert".to_string(), "cora".to_string(), "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.parquet".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "cora".to_string(),
        args: vec!["./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.parquet".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

/// Regression: an NRT file that ships DEPH but no PRES must still populate `pres`
/// (derived from DEPH) even when the region default has `has_deph_source = false`.
///
/// `GL_PR_CT_EXEC004K` is a DEPH-only file; converting it with the `nrt_ar` config
/// (whose default is `has_deph_source = false`) previously left `pres` and `deph`
/// entirely NaN because DEPH was never read. The converter now uses DEPH whenever
/// the file actually contains it.
#[test]
fn test_convert_nrt_ar_deph_only_generates_pres() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("ar_deph_only.parquet");
    let args = vec![
        "convert".to_string(),
        "nrt_ar".to_string(),
        "./tests/test_data/GL_PR_CT_EXEC004K.nc".to_string(),
        dest.to_str().unwrap().to_string(),
    ];
    handle_dispatch(&args).expect("conversion should succeed");

    let df = ParquetReader::new(std::fs::File::open(&dest).unwrap())
        .finish()
        .unwrap();
    assert!(df.height() > 0, "expected non-empty output");

    // pres must be fully populated (derived from DEPH), not all-NaN.
    let pres = df.column("pres").unwrap().f32().unwrap();
    let pres_nan = pres.into_iter().filter(|v| v.map_or(true, |x| x.is_nan())).count();
    assert_eq!(pres_nan, 0, "pres should be generated from DEPH, not NaN");

    // deph (the real source) must also be present.
    let deph = df.column("deph").unwrap().f32().unwrap();
    let deph_nan = deph.into_iter().filter(|v| v.map_or(true, |x| x.is_nan())).count();
    assert_eq!(deph_nan, 0, "deph should be read from the source");

    // Every pres value here is conversion-derived, so pres_conv must be 1.
    let conv = df.column("pres_conv").unwrap().i8().unwrap();
    assert!(
        conv.into_iter().all(|v| v == Some(1)),
        "all pres values are derived from DEPH, so pres_conv must be 1"
    );
}

// ── data mode (`temp_dm` / `psal_dm` / `pres_dm`) ───────────────────────────

const DM_COLS: [&str; 3] = ["temp_dm", "psal_dm", "pres_dm"];

/// Convert `src` with `target` into a temp dir and read it back.
fn convert_to_df(target: &str, src: &str) -> DataFrame {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("out.parquet");
    let args = vec![
        "convert".to_string(),
        target.to_string(),
        src.to_string(),
        dest.to_str().unwrap().to_string(),
    ];
    handle_dispatch(&args).expect("conversion should succeed");
    ParquetReader::new(std::fs::File::open(&dest).unwrap()).finish().unwrap()
}

/// Count of each value in a String column.
fn value_counts(df: &DataFrame, col: &str) -> std::collections::BTreeMap<String, usize> {
    let mut m = std::collections::BTreeMap::new();
    for v in df.column(col).unwrap().str().unwrap().into_iter() {
        *m.entry(v.unwrap_or("<null>").to_string()).or_insert(0) += 1;
    }
    m
}

/// Each `*_dm` column sits right after its `*_qc` column, in every converter.
#[test]
fn test_convert_dm_columns_follow_qc_columns() {
    for (target, src) in [
        ("nrt_ar", "./tests/test_data/AR_PR_CT_58KN.nc"),
        ("cora", "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc"),
    ] {
        let df = convert_to_df(target, src);
        let names: Vec<&str> = df.get_column_names().iter().map(|s| s.as_str()).collect();
        for p in ["temp", "psal", "pres"] {
            let qc = names.iter().position(|n| *n == format!("{p}_qc")).unwrap();
            assert_eq!(names[qc + 1], format!("{p}_dm"), "{target}: {p}_dm after {p}_qc");
            assert_eq!(df.column(&format!("{p}_dm")).unwrap().dtype(), &DataType::String);
        }
    }
}

/// `AR_PR_CT_58KN` ships `TEMP_DM` / `PSAL_DM` / `PRES_DM`: the per-observation
/// codes are read through, and only the valid R/A/D codes (or "" for fill) appear.
#[test]
fn test_convert_nrt_ar_dm_from_variables() {
    let df = convert_to_df("nrt_ar", "./tests/test_data/AR_PR_CT_58KN.nc");
    for col in DM_COLS {
        let counts = value_counts(&df, col);
        assert!(
            counts.keys().all(|k| ["R", "A", "D", ""].contains(&k.as_str())),
            "{col}: unexpected codes {counts:?}"
        );
        assert!(counts.get("D").copied().unwrap_or(0) > 0, "{col}: expected delayed-mode rows, got {counts:?}");
        assert!(counts.get("R").copied().unwrap_or(0) > 0, "{col}: expected real-time rows, got {counts:?}");
    }
}

/// `AR_PR_CT_ITP-71` has no `*_DM` variables (and `data_mode = "R"` on each
/// parameter), so every observation is real-time.
#[test]
fn test_convert_nrt_ar_dm_absent_defaults_to_r() {
    let df = convert_to_df("nrt_ar", "./tests/test_data/AR_PR_CT_ITP-71.nc");
    assert!(df.height() > 0);
    for col in DM_COLS {
        let counts = value_counts(&df, col);
        assert_eq!(counts.len(), 1, "{col}: {counts:?}");
        assert_eq!(counts.get("R"), Some(&df.height()), "{col}: {counts:?}");
    }
}

/// Pressure derived from DEPH carries a data mode too (inherited from DEPH).
#[test]
fn test_convert_nrt_dm_on_deph_derived_pressure() {
    let df = convert_to_df("nrt_gl", "./tests/test_data/GL_PR_CT_EXEC004K.nc");
    let conv = df.column("pres_conv").unwrap().i8().unwrap();
    let dm = df.column("pres_dm").unwrap().str().unwrap();
    let derived: Vec<Option<&str>> = conv
        .into_iter()
        .zip(dm.into_iter())
        .filter(|(c, _)| *c == Some(1))
        .map(|(_, d)| d)
        .collect();
    assert!(!derived.is_empty(), "fixture should have DEPH-derived pressure");
    assert!(derived.iter().all(|d| *d == Some("R")), "{derived:?}");
}

/// CORA has no `*_DM` variables; all three columns come from the per-profile
/// `DATA_MODE`, for both the current and the legacy format.
#[test]
fn test_convert_cora_dm_from_data_mode() {
    for (target, src) in [
        ("cora", "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc"),
        ("cora_legacy", "./tests/test_data/CO_DMQCGL01_20201005_PR_CT.nc"),
    ] {
        let df = convert_to_df(target, src);
        assert!(df.height() > 0);
        for col in DM_COLS {
            let counts = value_counts(&df, col);
            assert!(
                counts.keys().all(|k| ["R", "A", "D", ""].contains(&k.as_str())),
                "{target} {col}: unexpected codes {counts:?}"
            );
        }
        // One DATA_MODE per profile drives all three parameters.
        assert_eq!(value_counts(&df, "temp_dm"), value_counts(&df, "pres_dm"));
        assert_eq!(value_counts(&df, "temp_dm"), value_counts(&df, "psal_dm"));
    }
}
