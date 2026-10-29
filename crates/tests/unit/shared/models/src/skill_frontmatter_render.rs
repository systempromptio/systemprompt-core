use serde_yaml::{Mapping, Value};
use systemprompt_models::bridge::manifest::skill_frontmatter::{
    PLATFORM_OWNED_SKILL_KEYS, is_platform_owned_skill_key, render_passthrough_frontmatter,
};

fn mapping(pairs: &[(&str, &str)]) -> Mapping {
    pairs
        .iter()
        .map(|(k, v)| (Value::String((*k).into()), Value::String((*v).into())))
        .collect()
}

#[test]
fn absent_frontmatter_renders_nothing() {
    assert_eq!(render_passthrough_frontmatter(None).unwrap(), "");
}

#[test]
fn only_platform_owned_keys_render_nothing() {
    let owned: Vec<(&str, &str)> = PLATFORM_OWNED_SKILL_KEYS
        .iter()
        .map(|k| (*k, "x"))
        .collect();
    assert_eq!(
        render_passthrough_frontmatter(Some(&mapping(&owned))).unwrap(),
        ""
    );
}

#[test]
fn owned_keys_are_filtered_and_keys_are_sorted() {
    let authored = mapping(&[
        ("zeta", "1"),
        ("title", "dropped"),
        ("allowed-tools", "Read"),
        ("hosts", "dropped"),
        ("model", "sonnet"),
    ]);
    let rendered = render_passthrough_frontmatter(Some(&authored)).unwrap();
    assert_eq!(rendered, "allowed-tools: Read\nmodel: sonnet\nzeta: '1'\n");
}

#[test]
fn nested_mappings_render_in_sorted_order() {
    let mut inner = Mapping::new();
    inner.insert(Value::String("b".into()), Value::String("2".into()));
    inner.insert(Value::String("a".into()), Value::String("1".into()));
    let mut authored = Mapping::new();
    authored.insert(
        Value::String("model".into()),
        Value::String("sonnet".into()),
    );
    authored.insert(Value::String("metadata".into()), Value::Mapping(inner));
    let rendered = render_passthrough_frontmatter(Some(&authored)).unwrap();
    assert_eq!(rendered, "metadata:\n  a: '1'\n  b: '2'\nmodel: sonnet\n");
}

#[test]
fn non_string_keys_are_never_platform_owned() {
    assert!(is_platform_owned_skill_key(&Value::String("name".into())));
    assert!(!is_platform_owned_skill_key(&Value::String("model".into())));
    assert!(!is_platform_owned_skill_key(&Value::Bool(true)));
}
