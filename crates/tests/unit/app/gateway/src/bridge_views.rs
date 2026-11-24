use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_gateway::surface::views::BridgeReleaseSource;
use systemprompt_manifest::services::BridgeReleasesSpec;
use systemprompt_traits::{ExtensionSecret, SecretLookup};

#[derive(Debug)]
struct Lookup(BTreeMap<&'static str, &'static str>);

impl SecretLookup for Lookup {
    fn lookup(&self, name: &str) -> Option<ExtensionSecret> {
        self.0
            .get(name)
            .map(|value| ExtensionSecret::new((*value).to_owned()))
    }
}

fn source() -> BridgeReleaseSource {
    BridgeReleaseSource::new(Arc::new(Lookup(BTreeMap::from([
        ("release_token", "ghp_release"),
        ("anthropic_api_key", "sk-ant-provider"),
        ("blank", ""),
    ]))))
}

fn spec(token_secret: Option<&str>) -> BridgeReleasesSpec {
    serde_json::from_value(serde_json::json!({
        "repo": "systempromptio/bridge",
        "tag_prefix": "bridge-v",
        "token_secret": token_secret,
        "assets": {},
    }))
    .expect("a release spec")
}

#[test]
fn the_release_source_reads_the_token_its_spec_names() {
    assert_eq!(
        source().token(&spec(Some("release_token"))).as_deref(),
        Some("ghp_release")
    );
}

#[test]
fn a_spec_without_a_token_secret_reads_no_secret() {
    assert_eq!(source().token(&spec(None)), None);
}

#[test]
fn an_unset_or_blank_token_reads_as_absent() {
    assert_eq!(source().token(&spec(Some("missing"))), None);
    assert_eq!(source().token(&spec(Some("blank"))), None);
}
