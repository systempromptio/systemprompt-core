//! Admission refuses an unknown disabled id read from a variable by naming
//! the variable, never its value; a literal unknown id is named as written.

use serde_yaml::Value;
use systemprompt::CompositionError;
use systemprompt::reference;
use systemprompt_manifest::profile::profile_field_kind;
use systemprompt_manifest::{ExtensionsConfig, interpolate_yaml};

const SECRET: &str = "sk-live-kf9-5be21d";

fn disabled(entry: &str) -> ExtensionsConfig {
    let mut tree: Value =
        serde_yaml::from_str(&format!("extensions:\n  disabled: [\"{entry}\"]\n")).expect("yaml");
    interpolate_yaml(
        &mut tree,
        &|name| (name == "SECRETISH").then(|| SECRET.to_owned()),
        &profile_field_kind,
    )
    .expect("every variable is set");
    serde_yaml::from_value(tree.get("extensions").cloned().unwrap_or(Value::Null)).expect("an id")
}

fn refusal(extensions: ExtensionsConfig) -> String {
    let mut profile = (*systemprompt_test_fixtures::test_boot_profile()).clone();
    profile.extensions = extensions;
    let error = reference()
        .build(&profile)
        .expect_err("an unknown disabled id is refused");
    assert!(
        matches!(error, CompositionError::UnknownExtension(_)),
        "{error:?}"
    );
    error.to_string()
}

#[test]
fn an_unknown_disabled_id_from_a_variable_names_the_variable() {
    let text = refusal(disabled("${SECRETISH}"));
    assert!(text.contains("the value of ${SECRETISH}"), "{text}");
    assert!(!text.contains(SECRET), "{text}");
}

#[test]
fn a_literal_unknown_disabled_id_is_named() {
    let text = refusal(disabled("no-such-member-kf9"));
    assert!(text.contains("no-such-member-kf9"), "{text}");
}
