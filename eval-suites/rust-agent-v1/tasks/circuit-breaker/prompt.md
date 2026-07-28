Complete `CircuitBreaker::record_failure`. Consecutive failures must open the
breaker when the configured threshold is reached. A successful call resets
the failure count and closes the breaker. A threshold of zero must behave as
one.
