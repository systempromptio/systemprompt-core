use systemprompt_models::auth::JwtAudience;
use systemprompt_models::profile::{
    BundleVerification, FetchFailurePolicy, HttpsServicesSource, OciServicesSource,
    ServicesProfileConfig, ServicesSource, default_resource_audiences,
};
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

pub(crate) const KEY_B64: &str = "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29";

pub(crate) fn valid_key_b64() -> String {
    use base64::Engine;
    let raw = [7u8; 32];
    base64::engine::general_purpose::STANDARD.encode(raw)
}

pub(crate) fn local_profile() -> Profile {
    Profile {
        storage: Default::default(),
        name: "p".to_owned(),
        display_name: "Profile P".to_owned(),
        target: ProfileType::Local,
        site: SiteConfig {
            name: "Site".to_owned(),
            github_link: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_owned(),
            external_db_access: false,
            pool: None,
        },
        server: ServerConfig {
            host: "127.0.0.1".to_owned(),
            port: 8080,
            api_server_url: "http://localhost:8080".to_owned(),
            api_internal_url: "http://localhost:8080".to_owned(),
            api_external_url: "https://example.com".to_owned(),
            use_https: false,
            cors_allowed_origins: vec![],
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            instance_id: None,
            metrics_port: None,
            max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
            trusted_proxies: vec!["fc00::/7".parse().expect("cidr")],
        },
        paths: PathsConfig {
            system: "/tmp/system".to_owned(),
            services: "/tmp/services".to_owned(),
            bin: "/tmp/bin".to_owned(),
            web_path: None,
            storage: Some("/tmp/storage".to_owned()),
            geoip_database: None,
        },
        security: SecurityConfig {
            issuer: "https://issuer.test".to_owned(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
            audiences: vec![JwtAudience::Api],
            allowed_resource_audiences: default_resource_audiences(),
            allow_registration: true,
            login_page_url: None,
            signing_key_path: std::path::PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        governance: None,
        services: ServicesProfileConfig::default(),
        system_admin: SystemAdminConfig {
            username: "admin".to_owned(),
            email: None,
        },
    }
}

pub(crate) fn errors_of(profile: &Profile) -> String {
    profile
        .validate()
        .err()
        .map_or_else(String::new, |e| format!("{e}"))
}

fn https_block(url: &str, verify: BundleVerification) -> HttpsServicesSource {
    HttpsServicesSource {
        url: url.to_owned(),
        auth_secret: None,
        verify,
    }
}

fn oci_block(reference: &str, verify: BundleVerification) -> OciServicesSource {
    OciServicesSource {
        reference: reference.to_owned(),
        auth_secret: None,
        verify,
    }
}

fn https_source_at(name: &str, url: &str, verify: BundleVerification) -> ServicesSource {
    ServicesSource {
        name: name.to_owned(),
        https: Some(https_block(url, verify)),
        oci: None,
    }
}

fn https_source(name: &str, verify: BundleVerification) -> ServicesSource {
    https_source_at(name, "https://bundles.example.com/base.tar.gz", verify)
}

fn pinned() -> BundleVerification {
    BundleVerification {
        sha256: Some(KEY_B64.to_owned()),
        ed25519_public_keys: vec![],
    }
}

#[test]
fn default_services_config_is_the_bundled_tree() {
    let cfg = ServicesProfileConfig::default();
    assert!(cfg.is_identity());
    assert!(cfg.sources.is_empty());
    assert_eq!(cfg.on_fetch_failure, FetchFailurePolicy::UseLastGood);
}

#[test]
fn a_port_offset_or_a_source_breaks_identity() {
    let cfg = ServicesProfileConfig {
        port_offset: 10,
        ..ServicesProfileConfig::default()
    };
    assert!(!cfg.is_identity());

    let mut cfg = ServicesProfileConfig::default();
    cfg.sources.push(https_source("base", pinned()));
    assert!(!cfg.is_identity());
}

#[test]
fn yaml_round_trips_an_https_source() {
    let yaml = r#"
port_offset: 0
sources:
  - name: base
    https:
      url: https://bundles.example.com/base.tar.gz
      verify:
        sha256: "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29"
cache_dir: /app/cache
on_fetch_failure: fail_closed
"#;
    let cfg: ServicesProfileConfig = serde_yaml::from_str(yaml).expect("parse");
    assert_eq!(cfg.on_fetch_failure, FetchFailurePolicy::FailClosed);
    assert_eq!(cfg.cache_dir.as_deref(), Some("/app/cache"));
    let https = cfg.sources[0].https.as_ref().expect("https source");
    assert_eq!(https.url, "https://bundles.example.com/base.tar.gz");
    assert!(https.auth_secret.is_none());
    assert!(cfg.sources[0].oci.is_none());

    let back: ServicesProfileConfig =
        serde_yaml::from_str(&serde_yaml::to_string(&cfg).expect("serialize")).expect("reparse");
    assert_eq!(back.sources, cfg.sources);
}

#[test]
fn yaml_round_trips_an_oci_source() {
    let yaml = r#"
sources:
  - name: marketplace
    oci:
      reference: ghcr.io/org/services:v1
      auth_secret: registry_token
      verify:
        ed25519_public_keys: ["Bwc="]
"#;
    let cfg: ServicesProfileConfig = serde_yaml::from_str(yaml).expect("parse");
    let oci = cfg.sources[0].oci.as_ref().expect("oci source");
    assert_eq!(oci.reference, "ghcr.io/org/services:v1");
    assert_eq!(cfg.sources[0].auth_secret(), Some("registry_token"));
    assert!(cfg.sources[0].https.is_none());
}

#[test]
fn unknown_source_keys_are_rejected() {
    let yaml = r#"
sources:
  - name: base
    https:
      url: https://bundles.example.com/base.tar.gz
      insecure: true
"#;
    assert!(serde_yaml::from_str::<ServicesProfileConfig>(yaml).is_err());
}

#[test]
fn an_unknown_transport_block_is_rejected() {
    let yaml = r#"
sources:
  - name: base
    git:
      url: https://example.com/repo.git
"#;
    assert!(serde_yaml::from_str::<ServicesProfileConfig>(yaml).is_err());
}

#[test]
fn a_source_naming_no_transport_is_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![ServicesSource {
        name: "base".to_owned(),
        https: None,
        oci: None,
    }];
    assert!(errors_of(&profile).contains("requires exactly one of https or oci"));
}

#[test]
fn a_source_naming_both_transports_is_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![ServicesSource {
        name: "base".to_owned(),
        https: Some(https_block(
            "https://bundles.example.com/base.tar.gz",
            pinned(),
        )),
        oci: Some(oci_block("ghcr.io/org/services:v1", pinned())),
    }];
    assert!(errors_of(&profile).contains("requires exactly one of https or oci"));
}

#[test]
fn a_pinned_https_source_validates() {
    let mut profile = local_profile();
    profile.services.sources = vec![https_source("base", pinned())];
    assert_eq!(errors_of(&profile), "");
}

#[test]
fn duplicate_source_names_are_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![
        https_source("base", pinned()),
        https_source("base", pinned()),
    ];
    assert!(errors_of(&profile).contains("duplicate name 'base'"));
}

#[test]
fn a_source_with_no_verification_is_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![https_source("base", BundleVerification::default())];
    assert!(errors_of(&profile).contains("exactly one of sha256 or ed25519_public_keys"));
}

#[test]
fn a_source_with_both_digest_and_keys_is_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![https_source(
        "base",
        BundleVerification {
            sha256: Some(KEY_B64.to_owned()),
            ed25519_public_keys: vec![valid_key_b64()],
        },
    )];
    assert!(errors_of(&profile).contains("exactly one of sha256 or ed25519_public_keys"));
}

#[test]
fn a_malformed_pin_digest_is_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![https_source(
        "base",
        BundleVerification {
            sha256: Some("ABC".to_owned()),
            ed25519_public_keys: vec![],
        },
    )];
    assert!(errors_of(&profile).contains("64 lowercase hex"));
}

#[test]
fn a_signing_key_of_the_wrong_length_is_rejected() {
    use base64::Engine;
    let short = base64::engine::general_purpose::STANDARD.encode([1u8; 16]);
    let mut profile = local_profile();
    profile.services.sources = vec![https_source(
        "base",
        BundleVerification {
            sha256: None,
            ed25519_public_keys: vec![short],
        },
    )];
    assert!(errors_of(&profile).contains("decodes to 16 bytes, expected 32"));
}

#[test]
fn a_signing_key_that_is_not_base64_is_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![https_source(
        "base",
        BundleVerification {
            sha256: None,
            ed25519_public_keys: vec!["not base64!!".to_owned()],
        },
    )];
    assert!(errors_of(&profile).contains("is not valid base64"));
}

#[test]
fn a_signing_key_of_the_right_length_validates() {
    let mut profile = local_profile();
    profile.services.sources = vec![https_source(
        "base",
        BundleVerification {
            sha256: None,
            ed25519_public_keys: vec![valid_key_b64()],
        },
    )];
    assert_eq!(errors_of(&profile), "");
}

#[test]
fn a_plain_http_bundle_url_is_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![https_source_at(
        "base",
        "http://bundles.example.com/base.tar.gz",
        pinned(),
    )];
    assert!(errors_of(&profile).contains("is not fetchable"));
}

#[test]
fn a_link_local_bundle_url_is_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![https_source_at(
        "base",
        "https://169.254.169.254/base.tar.gz",
        pinned(),
    )];
    assert!(errors_of(&profile).contains("is not fetchable"));
}

#[test]
fn an_unqualified_oci_reference_is_rejected() {
    let mut profile = local_profile();
    profile.services.sources = vec![ServicesSource {
        name: "mk".to_owned(),
        https: None,
        oci: Some(oci_block("services:v1", pinned())),
    }];
    assert!(errors_of(&profile).contains("oci.reference is invalid"));
}

#[test]
fn a_relative_cache_dir_is_rejected() {
    let mut profile = local_profile();
    profile.services.cache_dir = Some("cache/bundles".to_owned());
    assert!(errors_of(&profile).contains("must be an absolute path"));
}

#[test]
fn a_cloud_cache_dir_outside_app_is_rejected() {
    let mut profile = local_profile();
    profile.target = ProfileType::Cloud;
    profile.services.cache_dir = Some("/var/lib/bundles".to_owned());
    assert!(errors_of(&profile).contains("services.cache_dir should start with /app"));
}

#[test]
fn an_absolute_cache_dir_validates_locally() {
    let mut profile = local_profile();
    profile.services.cache_dir = Some("/var/lib/systemprompt/bundles".to_owned());
    assert_eq!(errors_of(&profile), "");
}
