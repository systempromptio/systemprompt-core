//! `credentials.yaml`: kinds, secret detection, and the credential rules
//! `ResolvedCatalog::build` enforces.

use systemprompt_catalog_fixtures::{CatalogFixture, caller, opencode_go, personal, platform};
use systemprompt_identifiers::SecretName;
use systemprompt_manifest::services::catalog::{
    CatalogError, CredentialEntry, CredentialKind, CredentialRegistry, EndpointConfig,
    PlatformSecretKind, ProviderConfig,
};
use systemprompt_wire::WireProtocol;

#[test]
fn credentials_parse_as_platform_or_personal() {
    let yaml = "anthropic-key: { kind: platform, secret: anthropic }\nopencode-go: { kind: personal, label: \"OpenCode Go\" }\n";
    let registry: CredentialRegistry = serde_yaml::from_str(yaml).expect("credentials parse");
    let key = registry.get("anthropic-key").expect("declared");
    assert_eq!(key.kind, CredentialKind::Platform);
    assert_eq!(
        key.secret.as_ref().map(SecretName::as_str),
        Some("anthropic")
    );
    let go = registry.get("opencode-go").expect("declared");
    assert!(go.kind.is_personal());
    assert_eq!(go.label.as_deref(), Some("OpenCode Go"));
    let refused: Result<CredentialRegistry, _> =
        serde_yaml::from_str("x: { kind: user, secret: y }\n");
    assert!(refused.is_err(), "`user` is not a credential kind");
}

#[test]
fn a_platform_secret_is_a_service_account_only_when_it_says_so() {
    let sa = r#"{"type":"service_account","project_id":"p","private_key":"k"}"#;
    assert_eq!(
        PlatformSecretKind::detect(sa),
        PlatformSecretKind::ServiceAccount
    );
    assert_eq!(
        PlatformSecretKind::detect("sk-ant-123"),
        PlatformSecretKind::ApiKey
    );
    assert_eq!(
        PlatformSecretKind::detect(r#"{"type":"authorized_user"}"#),
        PlatformSecretKind::ApiKey
    );
}

fn with_credential(entry: CredentialEntry) -> CatalogFixture {
    CatalogFixture::new()
        .credential("c", entry)
        .provider("p", ProviderConfig::new("c"))
        .endpoint(
            "p",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://api.anthropic.com/v1"),
        )
        .model("m", systemprompt_catalog_fixtures::claude_model(200_000))
        .deploy("m", ["p/api"])
}

#[test]
fn a_platform_credential_needs_a_secret() {
    let entry = CredentialEntry {
        secret: None,
        ..platform("x")
    };
    let error = with_credential(entry).try_build().expect_err("refused");
    assert!(
        matches!(error, CatalogError::PlatformCredentialWithoutSecret { .. }),
        "{error}"
    );
}

#[test]
fn a_personal_credential_names_no_secret() {
    let entry = CredentialEntry {
        secret: Some(SecretName::new("shared")),
        ..personal("x")
    };
    let error = with_credential(entry).try_build().expect_err("refused");
    assert!(
        matches!(error, CatalogError::PersonalCredentialWithSecret { .. }),
        "{error}"
    );
}

#[test]
fn a_provider_must_name_a_declared_credential() {
    let fixture = with_credential(platform("x")).provider(
        "q",
        ProviderConfig {
            endpoints: systemprompt_catalog_fixtures::endpoints(&[(
                "api",
                EndpointConfig::new(WireProtocol::Anthropic, "https://api.anthropic.com/v1"),
            )]),
            ..ProviderConfig::new("ghost")
        },
    );
    let error = fixture.try_build().expect_err("refused");
    assert!(
        matches!(error, CatalogError::UnknownCredential { .. }),
        "{error}"
    );
}

#[test]
fn one_personal_credential_belongs_to_one_provider() {
    let fixture = opencode_go().provider(
        "opencode-go-copy",
        ProviderConfig {
            endpoints: systemprompt_catalog_fixtures::endpoints(&[(
                "chat",
                EndpointConfig::new(
                    WireProtocol::OpenAiChat,
                    systemprompt_catalog_fixtures::OPENCODE_GO_CHAT_URL,
                ),
            )]),
            ..ProviderConfig::new("opencode-go")
        },
    );
    let error = fixture.try_build().expect_err("refused");
    assert!(
        matches!(error, CatalogError::PersonalCredentialShared { .. }),
        "{error}"
    );
}

#[test]
fn every_endpoint_of_a_personal_credential_is_listed_for_verification() {
    let catalog = opencode_go().build();
    let mut urls: Vec<&str> = catalog
        .endpoints_for_credential("opencode-go")
        .map(|e| e.url.as_str())
        .collect();
    urls.sort_unstable();
    let mut expected = vec![
        systemprompt_catalog_fixtures::OPENCODE_GO_CHAT_URL,
        systemprompt_catalog_fixtures::OPENCODE_GO_MESSAGES_URL,
    ];
    expected.sort_unstable();
    assert_eq!(urls, expected);
}

#[test]
fn a_caller_credential_parses_and_resolves_on_the_vendors_own_api() {
    let registry: CredentialRegistry =
        serde_yaml::from_str("own-account: { kind: caller, label: Own account }\n")
            .expect("credentials parse");
    let entry = registry.get("own-account").expect("declared");
    assert_eq!(entry.kind, CredentialKind::Caller);
    assert!(entry.kind.is_member_paid() && !entry.kind.is_personal());

    let catalog = with_credential(caller("Own account")).build();
    let credential = catalog.credential("c").expect("resolved");
    assert!(credential.is_caller());
    assert!(credential.platform_secret().is_none());
}

#[test]
fn a_caller_credential_names_no_secret() {
    let entry = CredentialEntry {
        secret: Some(SecretName::new("anthropic")),
        ..caller("Own account")
    };
    let error = with_credential(entry).try_build().expect_err("refused");
    assert!(
        matches!(error, CatalogError::CallerCredentialWithSecret { .. }),
        "{error}"
    );
}

fn caller_on(endpoint: EndpointConfig) -> CatalogFixture {
    CatalogFixture::new()
        .credential("c", caller("Own account"))
        .provider("p", ProviderConfig::new("c"))
        .endpoint("p", "api", endpoint)
        .model("m", systemprompt_catalog_fixtures::claude_model(200_000))
        .deploy("m", ["p/api"])
}

#[test]
fn a_caller_credential_serves_any_wire_the_vendor_hosts_itself() {
    let catalog = caller_on(EndpointConfig::new(
        WireProtocol::OpenAiChat,
        "https://api.openai.com/v1",
    ))
    .build();
    assert!(catalog.credential("c").expect("resolved").is_caller());
}

#[test]
fn a_caller_credential_is_refused_off_the_vendors_own_api() {
    let error = caller_on(EndpointConfig::new(
        WireProtocol::Anthropic,
        "https://us-east5-aiplatform.googleapis.com/v1/projects/{project}/locations/us-east5/publishers/anthropic/models",
    ))
    .try_build()
    .expect_err("refused");
    assert!(
        matches!(error, CatalogError::CallerCredentialNotFirstParty { .. }),
        "{error}"
    );
}

#[test]
fn a_caller_endpoint_declares_no_accepted_betas() {
    let mut endpoint = EndpointConfig::new(WireProtocol::Anthropic, "https://api.anthropic.com/v1");
    endpoint.accepted_betas = Some(systemprompt_manifest::services::catalog::BetaSpec::List(
        std::collections::BTreeSet::new(),
    ));
    let error = caller_on(endpoint).try_build().expect_err("refused");
    assert!(
        matches!(error, CatalogError::CallerCredentialFiltersBetas { .. }),
        "{error}"
    );
}

#[test]
fn a_caller_credential_belongs_to_one_provider() {
    let error = with_credential(caller("Own account"))
        .provider("q", ProviderConfig::new("c"))
        .endpoint(
            "q",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://api.anthropic.com/v1"),
        )
        .try_build()
        .expect_err("refused");
    assert!(
        matches!(error, CatalogError::PersonalCredentialShared { .. }),
        "{error}"
    );
}

#[test]
fn a_caller_provider_sets_no_headers_of_its_own() {
    let mut provider = ProviderConfig::new("c");
    provider.user_agent = Some("deployment/1.0".to_owned());
    let error = CatalogFixture::new()
        .credential("c", caller("Own account"))
        .provider("p", provider)
        .endpoint(
            "p",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://api.anthropic.com/v1"),
        )
        .model("m", systemprompt_catalog_fixtures::claude_model(200_000))
        .deploy("m", ["p/api"])
        .try_build()
        .expect_err("refused");
    assert!(
        matches!(error, CatalogError::CallerProviderSetsHeaders { .. }),
        "{error}"
    );
}
