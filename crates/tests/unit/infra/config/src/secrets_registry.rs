//! Tests for the external secrets sources handed to the bootstrap.

use systemprompt_config::{
    DynSecretsProvider, EnvLookup, ExternalSecretsConfig, SecretsBootstrapError, SecretsDocument,
    SecretsProvider, SecretsSourceRegistration, SecretsSources, SharedSecretsProvider,
    build_external_provider,
};

struct StubProvider {
    region: String,
    token: Option<String>,
}

impl SecretsProvider for StubProvider {
    async fn fetch(&self) -> Result<SecretsDocument, SecretsBootstrapError> {
        let mut fields = serde_json::Map::new();
        fields.insert(
            "oauth_at_rest_pepper".to_owned(),
            serde_json::Value::String("p".repeat(40)),
        );
        fields.insert(
            "database_url".to_owned(),
            serde_json::Value::String("postgres://stub".to_owned()),
        );
        if let Some(token) = &self.token {
            fields.insert(
                "anthropic".to_owned(),
                serde_json::Value::String(token.clone()),
            );
        }
        Ok(SecretsDocument::new(fields))
    }

    fn describe(&self) -> String {
        format!("stub ({})", self.region)
    }
}

fn stub_factory(
    cfg: &ExternalSecretsConfig,
    env: EnvLookup,
) -> Result<SharedSecretsProvider, SecretsBootstrapError> {
    let region = cfg
        .config
        .get("region")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("default")
        .to_owned();
    Ok(Box::new(StubProvider {
        region,
        token: env("STUB_TOKEN"),
    }))
}

fn failing_factory(
    cfg: &ExternalSecretsConfig,
    _env: EnvLookup,
) -> Result<SharedSecretsProvider, SecretsBootstrapError> {
    Err(SecretsBootstrapError::External {
        kind: cfg.kind.clone(),
        source: "backend unreachable".into(),
    })
}

fn sources() -> SecretsSources {
    SecretsSources::new(vec![
        SecretsSourceRegistration {
            kind: "registry-test-stub",
            factory: stub_factory,
        },
        SecretsSourceRegistration {
            kind: "registry-test-failing",
            factory: failing_factory,
        },
        SecretsSourceRegistration {
            kind: "registry-test-stub",
            factory: stub_factory,
        },
    ])
}

fn external(kind: &str) -> ExternalSecretsConfig {
    let mut config = serde_json::Map::new();
    config.insert(
        "region".to_owned(),
        serde_json::Value::String("eu".to_owned()),
    );
    ExternalSecretsConfig {
        kind: kind.to_owned(),
        refresh_secs: None,
        config,
    }
}

#[tokio::test]
async fn a_registered_kind_is_built_with_its_config_and_env() {
    let env: EnvLookup = Box::new(|name| (name == "STUB_TOKEN").then(|| "tok".to_owned()));
    let provider =
        build_external_provider(&external("registry-test-stub"), env, &sources()).expect("built");
    assert_eq!(provider.describe(), "stub (eu)");
    let document = provider.fetch_boxed().await.expect("fetched");
    assert_eq!(
        document.key_names(),
        vec!["anthropic", "database_url", "oauth_at_rest_pepper"]
    );
    document.into_secrets().expect("document parses");
}

#[test]
fn an_unknown_kind_lists_every_registered_kind() {
    let Err(err) = build_external_provider(&external("nope"), Box::new(|_| None), &sources())
    else {
        panic!("unknown kind built");
    };
    match err {
        SecretsBootstrapError::UnknownExternalKind { kind, known } => {
            assert_eq!(kind, "nope");
            assert!(known.contains(&"registry-test-stub".to_owned()));
            assert!(known.contains(&"registry-test-failing".to_owned()));
        },
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn a_factory_error_is_returned_unchanged() {
    let Err(err) = build_external_provider(
        &external("registry-test-failing"),
        Box::new(|_| None),
        &sources(),
    ) else {
        panic!("failing factory built");
    };
    assert!(
        matches!(err, SecretsBootstrapError::External { ref kind, .. } if kind == "registry-test-failing")
    );
    assert!(err.to_string().contains("backend unreachable"));
}

#[test]
fn declared_kinds_are_sorted_and_unique() {
    assert_eq!(
        sources().kinds(),
        vec!["registry-test-failing", "registry-test-stub"]
    );
}

#[test]
fn no_declared_source_builds_no_external_kind() {
    let Err(err) = build_external_provider(
        &external("registry-test-stub"),
        Box::new(|_| None),
        &SecretsSources::default(),
    ) else {
        panic!("undeclared kind built");
    };
    assert!(
        matches!(err, SecretsBootstrapError::UnknownExternalKind { ref known, .. } if known.is_empty())
    );
}

#[test]
fn the_dyn_seam_covers_every_static_provider() {
    fn assert_dyn(_: &dyn DynSecretsProvider) {}
    assert_dyn(&StubProvider {
        region: String::new(),
        token: None,
    });
}
