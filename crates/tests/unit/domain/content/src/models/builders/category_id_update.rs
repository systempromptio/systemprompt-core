use systemprompt_content::models::CategoryIdUpdate;
use systemprompt_identifiers::CategoryId;

#[test]
fn from_none_produces_unchanged() {
    let value: Option<Option<CategoryId>> = None;
    let update: CategoryIdUpdate = value.into();
    assert!(matches!(update, CategoryIdUpdate::Unchanged));
}

#[test]
fn from_some_none_produces_clear() {
    let value: Option<Option<CategoryId>> = Some(None);
    let update: CategoryIdUpdate = value.into();
    assert!(matches!(update, CategoryIdUpdate::Clear));
}

#[test]
fn from_some_some_produces_set() {
    let id = CategoryId::new("tech");
    let value: Option<Option<CategoryId>> = Some(Some(id));
    let update: CategoryIdUpdate = value.into();
    match update {
        CategoryIdUpdate::Set(inner) => assert_eq!(inner.as_str(), "tech"),
        other => panic!("Expected Set variant, got {:?}", other),
    }
}
