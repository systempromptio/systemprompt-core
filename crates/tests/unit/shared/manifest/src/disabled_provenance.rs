//! An `extensions.disabled` entry interpolated from a variable keeps the
//! variable's name, so every refusal of it names "the value of ${NAME}" and
//! never the resolved value; a literal entry is named as written.

use std::path::Path;

use serde_yaml::Value;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::layered::parse_layer;
use systemprompt_manifest::profile::profile_field_kind;
use systemprompt_manifest::{DisabledExtension, ExtensionsConfig, interpolate_yaml};

const SECRET: &str = "sk-live-kf9-0f3a9c";

fn extensions(yaml: &str) -> Result<ExtensionsConfig, String> {
    let mut tree: Value = serde_yaml::from_str(yaml).expect("yaml");
    interpolate_yaml(
        &mut tree,
        &|name| (name == "KF9_SECRETISH").then(|| SECRET.to_owned()),
        &profile_field_kind,
    )
    .expect("every variable is set");
    let section = tree.get("extensions").cloned().unwrap_or(Value::Null);
    serde_yaml::from_value(section).map_err(|e| e.to_string())
}

#[test]
fn an_interpolated_entry_keeps_its_variable_and_a_literal_entry_none() {
    let config = extensions("extensions:\n  disabled: [\"${KF9_SECRETISH}\", mo-literal]\n")
        .expect("both entries are ids");
    assert_eq!(
        config.disabled,
        vec![
            DisabledExtension::interpolated(
                ExtensionId::new(SECRET),
                vec!["KF9_SECRETISH".to_owned()]
            ),
            DisabledExtension::literal(ExtensionId::new("mo-literal")),
        ]
    );
    assert_eq!(
        config.disabled[0].to_string(),
        "the value of ${KF9_SECRETISH}"
    );
    assert_eq!(config.disabled[1].to_string(), "mo-literal");
    assert!(config.is_disabled(&ExtensionId::new(SECRET)));
}

#[test]
fn an_interpolated_scalar_where_a_list_belongs_names_the_variable() {
    let error = extensions("extensions:\n  disabled: \"${KF9_SECRETISH}\"\n")
        .expect_err("a scalar is not a list");
    assert!(error.contains("${KF9_SECRETISH}"), "{error}");
    assert!(!error.contains(SECRET), "{error}");
    let literal = extensions("extensions:\n  disabled: mo-literal\n")
        .expect_err("negative control: a literal scalar is not a list either");
    assert!(literal.contains("not a list"), "{literal}");
}

#[test]
fn an_interpolated_value_that_is_no_id_names_the_variable() {
    let mut tree: Value =
        serde_yaml::from_str("extensions:\n  disabled: [\"${KF9_EMPTY:-}\"]\n").expect("yaml");
    interpolate_yaml(&mut tree, &|_| None, &profile_field_kind).expect("defaulted");
    let section = tree.get("extensions").cloned().unwrap_or(Value::Null);
    let error = serde_yaml::from_value::<ExtensionsConfig>(section)
        .expect_err("an empty id does not validate")
        .to_string();
    assert!(error.contains("${KF9_EMPTY}"), "{error}");
}

#[test]
fn the_boot_layer_records_the_variable_of_a_disabled_entry() {
    let var = "SP_KF9_DISABLED_SECRETISH_7f3a";
    unsafe { std::env::set_var(var, SECRET) };
    let layer = parse_layer(
        &format!("extensions:\n  disabled: [\"${{{var}}}\"]\n"),
        Path::new("/profiles/prod/profile.yaml"),
    );
    unsafe { std::env::remove_var(var) };
    let section = layer
        .expect("resolves")
        .get("extensions")
        .cloned()
        .unwrap_or(Value::Null);
    let config: ExtensionsConfig = serde_yaml::from_value(section).expect("an id");
    assert_eq!(config.disabled[0].variables(), [var.to_owned()]);
    assert_eq!(
        config.disabled[0].to_string(),
        format!("the value of ${{{var}}}")
    );
}

#[test]
fn the_debug_of_an_interpolated_entry_names_its_variable_and_not_its_value() {
    let config = extensions("extensions:\n  disabled: [\"${KF9_SECRETISH}\", mo-literal]\n")
        .expect("both entries are ids");
    let debug = format!("{:?}", config.disabled);
    assert!(!debug.contains(SECRET), "{debug}");
    assert!(debug.contains("KF9_SECRETISH"), "{debug}");
    assert!(debug.contains("mo-literal"), "{debug}");
    let pretty = format!("{:#?}", config);
    assert!(!pretty.contains(SECRET), "{pretty}");
}
