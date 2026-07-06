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
