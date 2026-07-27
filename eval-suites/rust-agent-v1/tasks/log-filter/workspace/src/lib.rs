use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    default: Level,
    targets: BTreeMap<String, Level>,
}

impl Filter {
    pub fn new(default: Level) -> Self {
        Self {
            default,
            targets: BTreeMap::new(),
        }
    }

    pub fn allows(&self, target: &str, level: Level) -> bool {
        level <= self.targets.get(target).copied().unwrap_or(self.default)
    }
}

pub fn parse_filter(_input: &str) -> Result<Filter, String> {
    Err("filter parsing is not implemented".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directly_constructed_filter_uses_default() {
        let filter = Filter::new(Level::Info);
        assert!(filter.allows("app", Level::Error));
        assert!(!filter.allows("app", Level::Debug));
    }
}
