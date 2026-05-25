use ctddump::{handle_dispatch, Config};

#[test]
fn test_header_nrt() {
    let args = vec![
        "header".to_string(),
        "nrt".to_string(),
        "./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(),
        "./tests/test_data/AR_PR_CT_ITP-71.yaml".to_string(),
    ];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "header".to_string(),
        target: "nrt".to_string(),
        args: vec![
            "./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(),
            "./tests/test_data/AR_PR_CT_ITP-71.yaml".to_string(),
        ],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_header_cora() {
    let args = vec![
        "header".to_string(),
        "cora".to_string(),
        "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(),
        "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.yaml".to_string(),
    ];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "header".to_string(),
        target: "cora".to_string(),
        args: vec![
            "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(),
            "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.yaml".to_string(),
        ],
    };

    assert_eq!(result.unwrap(), expected);
}
