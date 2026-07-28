Implement `retry_delays`. Return one delay per retry, starting at `base_ms`,
doubling for each subsequent retry, and capping each value at `max_ms`.
Handle arithmetic overflow without panicking and preserve zero-retry behavior.
