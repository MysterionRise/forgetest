use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkerOptions {
    pub create: bool,
    pub create_new: bool,
}

pub fn should_extract(_entry_path: &Path) -> bool {
    true
}

pub fn marker_options() -> MarkerOptions {
    MarkerOptions {
        create: true,
        create_new: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_source_file_is_extracted() {
        assert!(should_extract(Path::new("src/lib.rs")));
    }
}
