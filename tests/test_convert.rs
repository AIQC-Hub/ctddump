use ctddump::{handle_dispatch, Config};

#[test]
fn test_convert_nrt_head() {
    let args = vec!["convert".to_string(), "nrt_head".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.yaml".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "nrt_head".to_string(),
        args: vec!["./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.yaml".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

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
fn test_convert_cora_head() {
    let args = vec!["convert".to_string(), "cora_head".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.yaml".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "convert".to_string(),
        target: "cora_head".to_string(),
        args: vec!["./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.yaml".to_string()],
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
