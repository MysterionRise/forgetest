use std::path::Path;

use ft_safe_path::safe_join;

#[test]
fn rejects_parent_and_absolute_components() {
    let root = Path::new("/workspace");
    assert!(safe_join(root, Path::new("../secret")).is_err());
    assert!(safe_join(root, Path::new("src/../../secret")).is_err());
    assert!(safe_join(root, Path::new("/etc/passwd")).is_err());
    assert!(safe_join(root, Path::new("./src/lib.rs")).is_ok());
}
