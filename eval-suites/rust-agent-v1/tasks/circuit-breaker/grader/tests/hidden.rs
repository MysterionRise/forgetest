use ft_circuit_breaker::CircuitBreaker;

#[test]
fn opens_at_threshold_and_success_resets() {
    let mut breaker = CircuitBreaker::new(2);
    breaker.record_failure();
    assert!(!breaker.is_open());
    breaker.record_failure();
    assert!(breaker.is_open());
    assert_eq!(breaker.failures(), 2);
    breaker.record_success();
    assert!(!breaker.is_open());
    assert_eq!(breaker.failures(), 0);
}

#[test]
fn zero_threshold_opens_on_first_failure() {
    let mut breaker = CircuitBreaker::new(0);
    breaker.record_failure();
    assert!(breaker.is_open());
}
