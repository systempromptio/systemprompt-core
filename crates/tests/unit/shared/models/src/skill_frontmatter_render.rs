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
fn owned_keys_are_filtered_and_author_order_is_preserved() {
    let authored = mapping(&[
        ("zeta", "1"),
        ("title", "dropped"),
        ("allowed-tools", "Read"),
        ("hosts", "dropped"),
        ("model", "sonnet"),
    ]);
    let rendered = render_passthrough_frontmatter(Some(&authored)).unwrap();
    assert_eq!(rendered, "zeta: '1'\nallowed-tools: Read\nmodel: sonnet\n");
}

#[test]
fn non_string_keys_are_never_platform_owned() {
    assert!(is_platform_owned_skill_key(&Value::String("name".into())));
    assert!(!is_platform_owned_skill_key(&Value::String("model".into())));
    assert!(!is_platform_owned_skill_key(&Value::Bool(true)));
}
