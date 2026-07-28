#[derive(Debug)]
pub struct CircuitBreaker {
    threshold: u32,
    failures: u32,
    open: bool,
}

impl CircuitBreaker {
    pub fn new(threshold: u32) -> Self {
        Self {
            threshold,
            failures: 0,
            open: false,
        }
    }

    pub fn record_failure(&mut self) {}

    pub fn record_success(&mut self) {
        self.failures = 0;
        self.open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_breaker_is_closed() {
        assert!(!CircuitBreaker::new(3).is_open());
    }
}
