use ft_log_filter::{parse_filter, Level};

#[test]
fn parses_defaults_and_target_overrides() {
    let filter = parse_filter("warn,db=debug,DB=trace,api=error").unwrap();
    assert!(filter.allows("other", Level::Warn));
    assert!(!filter.allows("other", Level::Info));
    assert!(filter.allows("db", Level::Trace));
    assert!(!filter.allows("api", Level::Warn));
}

#[test]
fn rejects_malformed_directives() {
    assert!(parse_filter("info,,db=debug").is_err());
    assert!(parse_filter("=debug").is_err());
    assert!(parse_filter("verbose").is_err());
}
