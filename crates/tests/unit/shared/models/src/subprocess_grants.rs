use systemprompt_models::mcp::Deployment;
use systemprompt_models::subprocess::{
    ProviderSecret, SecretGrant, SecretGrantError, is_api_held_env_name,
};

fn internal(extra: &str) -> Result<Deployment, serde_yaml::Error> {
    serde_yaml::from_str(&format!(
        "binary: srv\nport: 5100\nenabled: true\ndisplay_in_web: false\noauth:\n  required: \
         false\n  scopes: []\n  audience: mcp\n  client_id: null\n{extra}"
    ))
}

#[test]
fn a_server_without_secrets_parses_to_an_empty_grant_list() {
    let deployment = internal("").expect("parses");
    assert!(deployment.secrets.is_empty());
    deployment.validate("docs").expect("validates");
}

#[test]
fn provider_and_custom_grants_parse() {
    let deployment = internal("secrets: [anthropic, \"custom:serp_api_key\"]\n").expect("parses");
    assert_eq!(
        deployment.secrets,
        vec![
            SecretGrant::Provider(ProviderSecret::Anthropic),
            SecretGrant::Custom("serp_api_key".to_owned()),
        ]
    );
    deployment.validate("research").expect("validates");
}

#[test]
fn an_unknown_grant_is_refused() {
    let err = internal("secrets: [mystery]\n").expect_err("unknown");
    assert!(err.to_string().contains("not grantable"), "{err}");
}

#[test]
fn the_pepper_seed_signing_keys_and_licence_secret_are_never_grantable() {
    for name in [
        "oauth_at_rest_pepper",
        "manifest_signing_secret_seed",
        "signing_key_pem",
        "signing_key_staged_pem",
        "signing_key_previous_keys",
        "licence_signing_secret",
    ] {
        assert_eq!(
            name.parse::<SecretGrant>(),
            Err(SecretGrantError::ApiHeld {
                name: name.to_owned()
            }),
            "{name}"
        );
        assert_eq!(
            format!("custom:{name}").parse::<SecretGrant>(),
            Err(SecretGrantError::ApiHeld {
                name: name.to_owned()
            }),
            "custom:{name}"
        );
        let err = internal(&format!("secrets: [{name}]\n")).expect_err(name);
        assert!(err.to_string().contains("held by the API"), "{err}");
    }
}

#[test]
fn a_custom_grant_naming_a_reserved_secret_resolves_to_that_secret() {
    assert_eq!(
        "custom:anthropic".parse::<SecretGrant>(),
        Ok(SecretGrant::Provider(ProviderSecret::Anthropic))
    );
}

#[test]
fn the_master_key_is_never_a_grant() {
    for raw in [
        "encryption_master_key",
        "ENCRYPTION_MASTER_KEY",
        "custom:encryption_master_key",
        "custom:ENCRYPTION_MASTER_KEY",
    ] {
        assert_eq!(
            raw.parse::<SecretGrant>(),
            Err(SecretGrantError::MasterKeyWithheld),
            "{raw}"
        );
    }
    let err = internal("secrets: [encryption_master_key]\n").expect_err("refused");
    assert!(err.to_string().contains("member cipher"), "{err}");
}

#[test]
fn the_database_urls_are_not_grants() {
    assert!(matches!(
        "database_url".parse::<SecretGrant>(),
        Err(SecretGrantError::AlwaysGranted { .. })
    ));
}

#[test]
fn a_custom_name_outside_the_charset_is_refused() {
    assert!(matches!(
        "custom:bad-name".parse::<SecretGrant>(),
        Err(SecretGrantError::InvalidCustomName { .. })
    ));
    assert!(matches!(
        "custom:".parse::<SecretGrant>(),
        Err(SecretGrantError::InvalidCustomName { .. })
    ));
}

#[test]
fn a_secret_routed_through_env_vars_is_refused() {
    for name in [
        "ANTHROPIC_API_KEY",
        "DATABASE_URL",
        "SIGNING_KEY_PEM",
        "OAUTH_AT_REST_PEPPER",
        "MANIFEST_SIGNING_SECRET_SEED",
        "ENCRYPTION_MASTER_KEY",
        "GITHUB_TOKEN",
        "VAULT_TOKEN",
        "VAULT_ROLE_ID",
        "VAULT_SECRET_ID",
    ] {
        let deployment = internal(&format!("env_vars: [{name}]\n")).expect("parses");
        let err = deployment.validate("srv").expect_err(name);
        assert!(err.to_string().contains("carries a secret"), "{err}");
    }
    let deployment = internal("env_vars: [RUST_LOG]\n").expect("parses");
    deployment.validate("srv").expect("non-secret passthrough");
    assert!(!is_api_held_env_name("RUST_LOG"));
}

#[test]
fn grants_on_an_external_server_are_refused() {
    let deployment: Deployment = serde_yaml::from_str(
        "server_type: external\nendpoint: https://mcp.example.com/mcp\nenabled: true\n\
         display_in_web: false\noauth:\n  required: false\n  scopes: []\n  audience: mcp\n  \
         client_id: null\nsecrets: [anthropic]\n",
    )
    .expect("parses");
    let err = deployment.validate("remote").expect_err("external");
    assert!(err.to_string().contains("never spawned"), "{err}");
}

#[test]
fn a_grant_round_trips_through_its_manifest_spelling() {
    for raw in ["anthropic", "custom:serp_api_key"] {
        let grant: SecretGrant = raw.parse().expect(raw);
        assert_eq!(grant.to_string(), raw);
    }
    assert_eq!(
        SecretGrant::Custom("serp_api_key".to_owned()).env_name(),
        "SERP_API_KEY"
    );
}
