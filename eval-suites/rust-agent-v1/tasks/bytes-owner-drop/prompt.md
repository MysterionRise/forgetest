This reduced fixture preserves the ownership bug fixed in `tokio-rs/bytes`
commit `36675436cc343fc0e828033278d668020bd897b9`. Fix `OwnedBytes::into_vec`
so converting into a vector releases the backing owner exactly once while
preserving the returned bytes.
