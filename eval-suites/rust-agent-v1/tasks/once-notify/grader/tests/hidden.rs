use std::sync::Arc;
use std::time::{Duration, Instant};

use ft_once_notify::OnceValue;

#[test]
fn set_wakes_an_existing_waiter() {
    let value = Arc::new(OnceValue::new());
    let waiter_value = Arc::clone(&value);
    let waiter = std::thread::spawn(move || {
        let started = Instant::now();
        let observed = waiter_value.wait_timeout(Duration::from_millis(500));
        (observed, started.elapsed())
    });
    std::thread::sleep(Duration::from_millis(40));
    value.set(42);

    let (observed, elapsed) = waiter.join().unwrap();
    assert_eq!(observed, Some(42));
    assert!(elapsed < Duration::from_millis(250), "waiter woke after {elapsed:?}");
}
