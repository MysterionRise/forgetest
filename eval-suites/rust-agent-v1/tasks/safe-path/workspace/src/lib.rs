use std::path::{Path, PathBuf};

pub fn safe_join(root: &Path, requested: &Path) -> Result<PathBuf, String> {
    if requested.is_absolute() {
        return Err("absolute paths are not allowed".into());
    }
    Ok(root.join(requested))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_normal_relative_path() {
        assert_eq!(
            safe_join(Path::new("/workspace"), Path::new("src/lib.rs")).unwrap(),
            Path::new("/workspace/src/lib.rs")
        );
    }
}
