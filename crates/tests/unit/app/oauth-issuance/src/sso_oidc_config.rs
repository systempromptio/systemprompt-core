use systemprompt_oauth_issuance::sso::oidc::{
    CONFIG_FILE, OidcClaimMapping, OidcConfig, OidcConfigError,
};

fn enabled(issuer: &str) -> OidcConfig {
    OidcConfig {
        enabled: true,
        issuer: issuer.to_owned(),
        client_id: systemprompt_identifiers::ClientId::new("console"),
        ..OidcConfig::default()
    }
}

fn validate(cfg: &OidcConfig) -> Result<(), OidcConfigError> {
    let secrets = systemprompt_test_fixtures::fixture_secret_store();
    cfg.validate(secrets.as_ref())
}

#[test]
fn defaults_describe_a_disabled_provider_with_standard_claims() {
    let cfg = OidcConfig::default();
    assert!(!cfg.enabled);
    assert_eq!(cfg.client_secret_key, "oidc_client_secret");
    assert_eq!(cfg.scopes, vec!["openid", "profile", "email"]);
    assert_eq!(cfg.clock_skew_seconds, 60);
    assert_eq!(cfg.login_path, "/admin/login");
    assert_eq!(cfg.default_return_to, "/admin");
    let claims = OidcClaimMapping::default();
    assert_eq!(
        (
            claims.email.as_str(),
            claims.name.as_str(),
            claims.groups.as_str()
        ),
        ("email", "name", "groups")
    );
    let parsed = OidcConfig::parse("{}").expect("empty file parses");
    assert_eq!(parsed.scopes, cfg.scopes);
    assert_eq!(parsed.claims.groups, "groups");
}

#[test]
fn load_treats_a_missing_file_as_off_and_reports_bad_files() {
    let root = tempfile::tempdir().unwrap();
    assert!(OidcConfig::load(root.path()).unwrap().is_none());

    let path = root.path().join(CONFIG_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "enabled: true\nissuer: https://idp.example.com\n").unwrap();
    let loaded = OidcConfig::load(root.path()).unwrap().expect("config");
    assert!(loaded.enabled);
    assert_eq!(loaded.issuer(), "https://idp.example.com");

    std::fs::write(&path, "enabled: [not a bool\n").unwrap();
    match OidcConfig::load(root.path()) {
        Err(OidcConfigError::Parse { path: reported, .. }) => assert_eq!(reported, path),
        other => panic!("expected a parse error, got {other:?}"),
    }

    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    match OidcConfig::load(root.path()) {
        Err(OidcConfigError::Read { path: reported, .. }) => assert_eq!(reported, path),
        other => panic!("expected a read error, got {other:?}"),
    }
}

#[test]
fn an_explicit_discovery_url_wins_and_jwks_hosts_merge() {
    let cfg = OidcConfig {
        discovery_url: Some("https://idp.example.com/custom/discovery".to_owned()),
        jwks_allowed_hosts: vec![
            " Keys.Example.COM ".to_owned(),
            "idp.example.com".to_owned(),
            String::new(),
            "keys.example.com".to_owned(),
        ],
        ..enabled("https://IDP.example.com/realm")
    };
    assert_eq!(
        cfg.discovery_url(),
        "https://idp.example.com/custom/discovery"
    );
    assert_eq!(
        cfg.jwks_hosts(),
        vec!["idp.example.com".to_owned(), "keys.example.com".to_owned()]
    );
    assert!(enabled("not a url").jwks_hosts().is_empty());
}

#[test]
fn validation_refuses_each_missing_or_unsafe_field() {
    assert!(matches!(
        validate(&enabled("  ")),
        Err(OidcConfigError::Missing("issuer"))
    ));
    let bad_discovery = OidcConfig {
        discovery_url: Some("http://idp.example.com/discovery".to_owned()),
        ..enabled("https://idp.example.com")
    };
    assert!(matches!(
        validate(&bad_discovery),
        Err(OidcConfigError::Url {
            field: "discovery_url",
            ..
        })
    ));
    let no_key = OidcConfig {
        client_secret_key: " ".to_owned(),
        ..enabled("https://idp.example.com")
    };
    assert!(matches!(
        validate(&no_key),
        Err(OidcConfigError::Missing("client_secret_key"))
    ));
    let absent = OidcConfig {
        client_secret_key: "oidc_secret_not_in_the_store".to_owned(),
        ..enabled("https://idp.example.com")
    };
    match validate(&absent) {
        Err(error @ OidcConfigError::MissingSecret { .. }) => assert_eq!(
            error.to_string(),
            "OIDC client secret `oidc_secret_not_in_the_store` is missing from the secrets store"
        ),
        other => panic!("expected a missing secret, got {other:?}"),
    }
    assert_eq!(
        OidcConfigError::Missing("issuer").to_string(),
        "OIDC `issuer` is required when the provider is enabled"
    );
}
