use std::path::Path;

use ft_cargo_ok_extraction::{marker_options, should_extract};

#[test]
fn archive_cannot_supply_the_success_marker() {
    assert!(!should_extract(Path::new(".cargo-ok")));
    assert!(!should_extract(Path::new("nested/.cargo-ok")));
    assert!(should_extract(Path::new(".cargo-ok.backup")));
    let options = marker_options();
    assert!(options.create_new);
    assert!(!options.create);
}
