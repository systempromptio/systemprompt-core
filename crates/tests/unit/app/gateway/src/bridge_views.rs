use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_gateway::surface::views::{
    BridgeReleaseSource, GatewaySettingsSource, GatewaySettingsView,
};
use systemprompt_identifiers::UserId;
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
        ("signing_key_pem", "-----BEGIN PRIVATE KEY-----"),
        ("anthropic", "sk-ant-platform"),
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

#[test]
fn a_spec_naming_a_built_in_secret_reads_nothing() {
    for name in [
        "signing_key_pem",
        "database_url",
        "oauth_at_rest_pepper",
        "anthropic",
    ] {
        assert_eq!(source().token(&spec(Some(name))), None, "{name}");
    }
}

fn settings() -> GatewaySettingsView {
    let config = systemprompt_test_fixtures::test_config();
    GatewaySettingsView::new(GatewaySettingsSource {
        config: &config,
        system_admin: UserId::new("admin"),
        services_root: PathBuf::from("/services"),
        secrets: Arc::new(Lookup(BTreeMap::from([
            ("release_token", "ghp_release"),
            ("signing_key_pem", "-----BEGIN PRIVATE KEY-----"),
            ("database_url", "postgres://db"),
            ("anthropic", "sk-ant-platform"),
        ]))),
    })
}

#[test]
fn the_settings_view_answers_custom_secrets_and_provider_keys_only() {
    let view = settings();
    assert!(view.credential_configured("release_token"));
    assert!(view.credential_configured("anthropic"));
    assert!(!view.credential_configured("signing_key_pem"));
    assert!(!view.credential_configured("database_url"));
}
