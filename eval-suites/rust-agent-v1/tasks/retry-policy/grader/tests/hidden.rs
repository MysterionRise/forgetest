use ft_retry_policy::retry_delays;

#[test]
fn delays_double_and_cap() {
    assert_eq!(retry_delays(10, 25, 5), vec![10, 20, 25, 25, 25]);
    assert_eq!(retry_delays(u64::MAX - 1, u64::MAX, 3), vec![
        u64::MAX - 1,
        u64::MAX,
        u64::MAX,
    ]);
}
