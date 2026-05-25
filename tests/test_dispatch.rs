use ctddump::handle_dispatch;

#[test]
fn test_handle_dispatch_concat_missing_args_is_error() {
    // concat requires <src_dir> and <output>; omitting them must be a parse error
    let args = vec!["concat".to_string()];
    assert!(handle_dispatch(&args).is_err());
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
