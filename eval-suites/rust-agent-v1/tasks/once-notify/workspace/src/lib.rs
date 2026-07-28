use std::sync::{Condvar, Mutex};
use std::time::Duration;

pub struct OnceValue<T> {
    value: Mutex<Option<T>>,
    ready: Condvar,
}

impl<T: Clone> OnceValue<T> {
    pub fn new() -> Self {
        Self {
            value: Mutex::new(None),
            ready: Condvar::new(),
        }
    }

    pub fn set(&self, value: T) {
        *self.value.lock().expect("value lock poisoned") = Some(value);
    }

    pub fn wait_timeout(&self, timeout: Duration) -> Option<T> {
        let guard = self.value.lock().expect("value lock poisoned");
        let (guard, _) = self
            .ready
            .wait_timeout_while(guard, timeout, |value| value.is_none())
            .expect("value lock poisoned");
        guard.clone()
    }
}

impl<T: Clone> Default for OnceValue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialized_value_is_immediately_available() {
        let value = OnceValue::new();
        value.set(7);
        assert_eq!(value.wait_timeout(Duration::from_millis(1)), Some(7));
    }
}
