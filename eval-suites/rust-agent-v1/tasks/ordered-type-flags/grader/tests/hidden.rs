use ft_ordered_type_flags::{selected_types, TypeChange};

#[test]
fn later_change_for_same_type_wins() {
    let included = selected_types(&[
        TypeChange::Exclude("lock".into()),
        TypeChange::Include("lock".into()),
    ]);
    assert!(included.contains("lock"));

    let excluded = selected_types(&[
        TypeChange::Include("rust".into()),
        TypeChange::Exclude("rust".into()),
    ]);
    assert!(!excluded.contains("rust"));
}
