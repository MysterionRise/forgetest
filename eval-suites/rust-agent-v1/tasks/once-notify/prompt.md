Fix `OnceValue::set` so threads waiting in `wait_timeout` are promptly woken
after initialization. Preserve the one-value API and do not busy-wait.
