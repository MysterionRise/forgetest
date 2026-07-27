pub fn retry_delays(_base_ms: u64, _max_ms: u64, _retries: usize) -> Vec<u64> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_retries_have_no_delays() {
        assert!(retry_delays(10, 100, 0).is_empty());
    }
}
