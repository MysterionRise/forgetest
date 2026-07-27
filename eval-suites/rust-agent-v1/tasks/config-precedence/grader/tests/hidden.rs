use ft_config_precedence::effective_timeout;

#[test]
fn more_explicit_sources_win() {
    assert_eq!(effective_timeout(Some(5), Some(10), Some(20), 30), 5);
    assert_eq!(effective_timeout(None, Some(10), Some(20), 30), 10);
}
