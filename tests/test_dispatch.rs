use ctddump::{handle_dispatch, Config};

#[test]
fn test_handle_dispatch_concat() {
    let args = vec!["concat".to_string(), "--arg1=val1".to_string()];
    let result = handle_dispatch(&args);

    let expected = Config {
        module: "concat".to_string(),
        target: "".to_string(),
        args: vec!["--arg1=val1".to_string()],
    };

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_handle_dispatch_unknown_module() {
    let args = vec!["unknown".to_string()];
    let result = handle_dispatch(&args);
    assert!(result.is_err());
}

#[test]
fn test_handle_dispatch_no_module() {
    let args: Vec<String> = vec![];
    let result = handle_dispatch(&args);
    assert!(result.is_err());
}
