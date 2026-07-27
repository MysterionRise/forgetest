use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeChange {
    Include(String),
    Exclude(String),
}

pub fn selected_types(changes: &[TypeChange]) -> BTreeSet<String> {
    let mut selected = BTreeSet::new();
    for change in changes {
        if let TypeChange::Include(name) = change {
            selected.insert(name.clone());
        }
    }
    for change in changes {
        if let TypeChange::Exclude(name) = change {
            selected.remove(name);
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonconflicting_changes_are_applied() {
        let result = selected_types(&[
            TypeChange::Include("rust".into()),
            TypeChange::Exclude("toml".into()),
        ]);
        assert!(result.contains("rust"));
        assert!(!result.contains("toml"));
    }
}
