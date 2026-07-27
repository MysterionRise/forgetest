use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ft_bytes_owner_drop::OwnedBytes;

struct Owner(Arc<AtomicUsize>);

impl Drop for Owner {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn conversion_releases_the_owner_once() {
    let drops = Arc::new(AtomicUsize::new(0));
    let bytes = OwnedBytes::new(vec![1, 2, 3], Owner(Arc::clone(&drops)));
    assert_eq!(bytes.into_vec(), vec![1, 2, 3]);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
