//! `extensions.secrets` grants a member custom-secret names and prefixes; an
//! empty name, an empty prefix or a bare `*` is refused when the profile
//! parses.

use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::ExtensionsConfig;

fn extensions(yaml: &str) -> Result<ExtensionsConfig, String> {
    serde_yaml::from_str(yaml).map_err(|e| e.to_string())
}

#[test]
fn a_member_is_granted_its_listed_names_and_prefixes_and_any_other_member_none() {
    let config = extensions("secrets:\n  acme-portal: [acme_api_key, \"acme_*\"]\n")
        .expect("the grants parse");
    let granted: Vec<String> = config
        .secret_grants(&ExtensionId::new("acme-portal"))
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(granted, ["acme_api_key", "acme_*"]);
    assert!(config.secret_grants(&ExtensionId::new("other")).is_empty());
    assert!(
        extensions("disabled: []\n")
            .expect("parses")
            .secrets
            .is_empty()
    );
}

#[test]
fn a_bare_wildcard_an_empty_name_and_an_inner_wildcard_are_refused() {
    for grant in ["\"*\"", "\"\"", "\"a*b\""] {
        let yaml = format!("secrets:\n  acme-portal: [{grant}]\n");
        assert!(extensions(&yaml).is_err(), "{grant} is refused");
    }
}
