//! An install reads a custom secret only when it declares the name and the
//! operator grants it in the profile (`extensions.secrets`): a declaration no
//! grant covers fails admission typed, a grant without a declaration reads
//! nothing, a name a core member, the profile or the platform references is
//! refused even when granted, a prefix grant is matched byte-exactly (the
//! store's case-swap fallback cannot widen it), and a name a services reload
//! starts to reference is refused from then on.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::{FactoryError, ProvideError, Resolved};
use systemprompt_identifiers::SecretName;
use systemprompt_manifest::profile::{
    BundleVerification, GcsCredentials, HttpsServicesSource, ProfileSecretReferences,
    ServicesSource,
};
use systemprompt_manifest::services::{ServicesConfig, ServicesView, SlackAppConfig};
use systemprompt_models::mcp::deployment::ConnectorConfig;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{AppContext, Kernel, KernelError};
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, probe_admitted, test_database_url,
};
use systemprompt_traits::{
    BackgroundTasks, CacheDomain, CacheEpochs, ConfigVersionSource, ConfigWatch, ExtensionSecret,
    MCP_CREDENTIAL_BROKER_SECRET, ReloadOutcome, RepositoryError, SECRET_SEED_BYTES,
    SecretReferences, SecretStore, SecretStoreError, SecretsStatus, WatchSignals,
};

const SLACK_SIGNING_REF: &str = "slack_signing_secret";
const OWN: &str = "fx_install_secret";
const LATE: &str = "fx_late_secret";
const BUCKET_KEY: &str = "fx_bucket_key";
const BUNDLE_AUTH: &str = "fx_bundle_auth";
const CONNECTOR_CLIENT_SECRET: &str = "fx_connector_client_secret";
const GITHUB_APP_KEY: &str = "fx_github_app_key";

#[derive(Debug)]
struct Store(BTreeMap<&'static str, &'static str>);

#[async_trait::async_trait]
impl SecretStore for Store {
    fn secret_exact(&self, name: &str) -> Option<ExtensionSecret> {
        self.0
            .get(name)
            .map(|v| ExtensionSecret::new((*v).to_owned()))
    }

    fn secret(&self, name: &str) -> Option<ExtensionSecret> {
        self.secret_exact(name)
            .or_else(|| self.secret_exact(&name.to_uppercase()))
            .or_else(|| self.secret_exact(&name.to_lowercase()))
    }

    fn key_names(&self) -> Result<Vec<String>, SecretStoreError> {
        Ok(self.0.keys().map(|k| (*k).to_owned()).collect())
    }

    fn manifest_signing_secret_seed(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        Ok([0; SECRET_SEED_BYTES])
    }

    fn encryption_master_key(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        Ok([0; SECRET_SEED_BYTES])
    }

    fn status(&self) -> SecretsStatus {
        SecretsStatus {
            source: "fixture".to_owned(),
            generation: 1,
            initialised_at: chrono::Utc::now(),
            last_reload_at: None,
            last_error: None,
        }
    }

    fn refresh_interval(&self) -> Option<Duration> {
        None
    }

    async fn reload(&self) -> Result<ReloadOutcome, SecretStoreError> {
        Ok(ReloadOutcome {
            generation: 1,
            changed_keys: Vec::new(),
            swapped: false,
        })
    }
}

fn store() -> Arc<dyn SecretStore> {
    Arc::new(Store(BTreeMap::from([
        (SLACK_SIGNING_REF, "slack-signing"),
        ("slack_bot_token", "slack-bot"),
        (OWN, "mine"),
        (LATE, "late"),
        ("jwt_secret", "jwt"),
        ("LICENCE_SIGNING_SECRET", "licence"),
        (MCP_CREDENTIAL_BROKER_SECRET, "broker"),
        (BUCKET_KEY, "bucket"),
        (BUNDLE_AUTH, "bundle"),
        (CONNECTOR_CLIENT_SECRET, "connector-client"),
        (GITHUB_APP_KEY, "github-app"),
        ("acme_x", "acme"),
        ("acmex", "acmex"),
        ("ACME_x", "acme-upper"),
        ("ACME_Y", "acme-swapped"),
    ])))
}

fn slack_app(signing_ref: &str) -> SlackAppConfig {
    serde_yaml::from_str(&format!(
        "workspace_id: T0FXFX\nsigning_secret_ref: {signing_ref}\nbot_token_ref: slack_bot_token\n"
    ))
    .expect("slack app parses")
}

fn with_slack(base: &ServicesConfig, signing_ref: &str) -> ServicesConfig {
    let mut config = base.clone();
    config
        .slack_apps
        .insert("fx_reference".to_owned(), slack_app(signing_ref));
    config
}

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

fn reference_profile() -> ProfileSecretReferences {
    let mut profile = (*systemprompt_test_fixtures::test_boot_profile()).clone();
    profile.storage.credentials = Some(GcsCredentials::Secret(SecretName::new(BUCKET_KEY)));
    profile.services.sources = vec![ServicesSource {
        name: "fx-bundle".to_owned(),
        https: Some(HttpsServicesSource {
            url: "https://bundles.example.test/services".to_owned(),
            auth_secret: Some(BUNDLE_AUTH.to_owned()),
            verify: BundleVerification::default(),
        }),
        oci: None,
    }];
    ProfileSecretReferences::of(&profile)
}

fn seed(ctx: &AppContext, services: ServicesView) -> Dependencies {
    let mut state = ctx.kernel().state().clone();
    state.insert(services);
    state.insert_sealed(store());
    state.insert_sealed(reference_profile());
    state
}

fn with_connector(base: &ServicesConfig) -> ServicesConfig {
    let mut config = base.clone();
    let connector: ConnectorConfig = serde_yaml::from_str(&format!(
        "client_id_secret: fx_connector_client_id\nclient_secret: {CONNECTOR_CLIENT_SECRET}\nprivate_key_secret: {GITHUB_APP_KEY}\n"
    ))
    .expect("connector parses");
    config.mcp_servers.insert(
        "fx-connector".to_owned(),
        systemprompt_models::mcp::Deployment {
            connector: Some(connector),
            server_type: systemprompt_models::mcp::McpServerType::Internal,
            binary: Some("fx-bin".to_owned()),
            package: None,
            port: Some(5999),
            endpoint: None,
            enabled: false,
            display_in_web: false,
            dev_only: false,
            schemas: vec![],
            oauth: systemprompt_models::mcp::OAuthRequirement {
                required: false,
                scopes: vec![],
                audience: systemprompt_models::auth::JwtAudience::Mcp,
                client_id: None,
                ema: false,
                service_only: false,
            },
            tools: std::collections::HashMap::new(),
            model_config: None,
            env_vars: vec![],
            secrets: Vec::new(),
            external_auth: None,
            headers: Default::default(),
            tool_policy: None,
            description: None,
            removable: true,
            toolsets: Vec::new(),
        },
    );
    config
}

fn reference_services(ctx: &AppContext) -> ServicesView {
    let base = ctx
        .kernel()
        .state()
        .get::<ServicesView>()
        .expect("the services view is a kernel seed")
        .clone();
    ServicesView {
        config: ConfigWatch::new(with_connector(&with_slack(
            &base.config(),
            SLACK_SIGNING_REF,
        ))),
        catalog: base.catalog,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Read {
    own: Option<String>,
    slack: Option<String>,
    late: Option<String>,
}

fn read(r: &Resolved<'_>) -> Result<Read, ProvideError> {
    let secrets = r.secrets()?;
    let value = |name: &str| secrets.secret(name).map(|s| s.expose().to_owned());
    Ok(Read {
        own: value(OWN),
        slack: value(SLACK_SIGNING_REF),
        late: value(LATE),
    })
}

fn declaring(id: &'static str, names: &[&str]) -> Probe {
    let names: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
    Probe::new(id, move |c| {
        for name in &names {
            c.secret(SecretName::new(name.as_str()));
        }
        c.provide(read);
    })
}

fn granted_declaring(id: &'static str, names: &[&str]) -> Probe {
    declaring(id, names).granted(names)
}

fn resolve(state: Dependencies, probes: Vec<Probe>) -> Result<Kernel, KernelError> {
    let mut kernel = Kernel::detached(BackgroundTasks::new(), state);
    kernel.resolve(&probe_admitted(ComposedParts {
        probes,
        ..ComposedParts::default()
    }))?;
    Ok(kernel)
}

#[tokio::test]
async fn an_install_reads_its_declared_secret_and_never_the_slack_signing_secret() {
    let ctx = context().await;
    let kernel = resolve(
        seed(&ctx, reference_services(&ctx)),
        vec![declaring("fx-install", &[OWN]).granted(&[OWN, LATE, SLACK_SIGNING_REF])],
    )
    .expect("an install declaring only its own granted secret is admitted");
    let read = kernel.state().get::<Read>().expect("the install's value");
    assert_eq!(read.own.as_deref(), Some("mine"));
    assert_eq!(
        read.slack, None,
        "the Slack signing secret is the slack member's, never an install's read"
    );
    assert_eq!(
        read.late, None,
        "a granted name the install did not declare reads nothing"
    );
}

#[tokio::test]
async fn an_install_declaring_a_secret_no_grant_covers_fails_admission_typed() {
    let ctx = context().await;
    for grants in [
        &[][..],
        &[LATE][..],
        &["fx_install_secret*"][..],
        &["fx_installx_*"][..],
        &["FX_INSTALL_SECRET"][..],
    ] {
        let error = resolve(
            seed(&ctx, reference_services(&ctx)),
            vec![declaring("fx-install", &[OWN]).granted(grants)],
        )
        .expect_err("a declaration with no covering grant is refused");
        assert!(
            matches!(
                &error,
                KernelError::UngrantedSecret { owner, pattern }
                    if owner.as_str() == "fx-install" && pattern.to_string() == OWN
            ),
            "{grants:?}: {error:?}"
        );
    }
}

#[tokio::test]
async fn a_prefix_grant_covers_its_names_byte_exactly_and_never_through_the_case_swap() {
    let ctx = context().await;
    let kernel = resolve(
        seed(&ctx, reference_services(&ctx)),
        vec![
            Probe::new("fx-acme", |c| {
                c.secret_prefix("acme_");
                c.provide(|r: &Resolved<'_>| -> Result<LateSecrets, ProvideError> {
                    Ok(LateSecrets(r.secrets()?))
                });
            })
            .granted(&["acme_*"]),
        ],
    )
    .expect("a prefix declaration under a prefix grant is admitted");
    let secrets = kernel.state().get::<LateSecrets>().expect("held").0.clone();
    let value = |name: &str| secrets.secret(name).map(|s| s.expose().to_owned());
    assert_eq!(value("acme_x").as_deref(), Some("acme"));
    assert_eq!(value("acmex"), None, "acme_* does not cover acmex");
    assert_eq!(value("ACME_x"), None, "acme_* does not cover ACME_x");
    assert_eq!(value("acme_"), None, "a prefix covers only a longer name");
    assert_eq!(
        value("acme_y"),
        None,
        "the store's case-swap fallback does not serve ACME_Y for a granted acme_y"
    );
}

#[tokio::test]
async fn a_granted_prefix_still_reads_no_core_referenced_name() {
    let ctx = context().await;
    let kernel = resolve(
        seed(&ctx, reference_services(&ctx)),
        vec![
            Probe::new("fx-wide", |c| {
                for prefix in ["fx_", "mcp_", "slack_"] {
                    c.secret_prefix(prefix);
                }
                c.provide(|r: &Resolved<'_>| -> Result<LateSecrets, ProvideError> {
                    Ok(LateSecrets(r.secrets()?))
                });
            })
            .granted(&["fx_*", "mcp_*", "slack_*"]),
        ],
    )
    .expect("the prefixes are granted");
    let secrets = kernel.state().get::<LateSecrets>().expect("held").0.clone();
    assert_eq!(
        secrets
            .secret(OWN)
            .map(|s| s.expose().to_owned())
            .as_deref(),
        Some("mine")
    );
    for name in [
        BUCKET_KEY,
        BUNDLE_AUTH,
        CONNECTOR_CLIENT_SECRET,
        GITHUB_APP_KEY,
        MCP_CREDENTIAL_BROKER_SECRET,
        SLACK_SIGNING_REF,
        "slack_bot_token",
    ] {
        assert!(
            secrets.secret(name).is_none(),
            "{name} is core-referenced and refused under a grant"
        );
    }
}

fn refused(error: &KernelError, owner: &str, name: &str) -> bool {
    matches!(
        error,
        KernelError::ReservedSecret { owner: o, name: n } if o.as_str() == owner && n.to_string() == name
    )
}

#[tokio::test]
async fn an_install_declaring_a_core_referenced_name_fails_admission_typed() {
    let ctx = context().await;
    for name in [
        SLACK_SIGNING_REF,
        "SLACK_BOT_TOKEN",
        "jwt_secret",
        "LICENCE_SIGNING_SECRET",
        "encryption_master_key",
        "oauth_at_rest_pepper",
        "signing_key_pem",
        "database_url",
        BUCKET_KEY,
        BUNDLE_AUTH,
        CONNECTOR_CLIENT_SECRET,
        GITHUB_APP_KEY,
        "fx_connector_client_id",
        MCP_CREDENTIAL_BROKER_SECRET,
    ] {
        let error = resolve(
            seed(&ctx, reference_services(&ctx)),
            vec![granted_declaring("fx-install", &[name])],
        )
        .expect_err("a core-referenced name is refused at admission, granted or not");
        assert!(refused(&error, "fx-install", name), "{name}: {error:?}");
    }
}

#[derive(Debug)]
struct Reserves(&'static str);

impl SecretReferences for Reserves {
    fn references(&self, name: &str) -> bool {
        self.0.eq_ignore_ascii_case(name)
    }
}

#[tokio::test]
async fn a_name_a_member_contributes_as_referenced_is_refused_to_an_install() {
    let ctx = context().await;
    let reserving = Probe::new("fx-reserver", |c| {
        c.secret_references(
            |_: &Resolved<'_>| -> Result<Arc<dyn SecretReferences>, FactoryError> {
                Ok(Arc::new(Reserves(OWN)))
            },
        );
    });
    let error = resolve(
        seed(&ctx, reference_services(&ctx)),
        vec![reserving, granted_declaring("fx-install", &[OWN])],
    )
    .expect_err("a contributed reference is refused");
    assert!(refused(&error, "fx-install", OWN), "{error:?}");
}

#[derive(Default)]
struct Version(std::sync::atomic::AtomicI64);

#[async_trait::async_trait]
impl ConfigVersionSource for Version {
    async fn current_version(&self) -> Result<i64, RepositoryError> {
        Ok(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }
}

#[tokio::test]
async fn a_name_a_services_reload_starts_to_reference_is_refused_from_then_on() {
    let ctx = context().await;
    let services = reference_services(&ctx);
    let reloaded = with_slack(&services.config(), LATE);
    let epochs = CacheEpochs::new();
    let tasks = BackgroundTasks::new();
    services.config.spawn(
        &tasks,
        WatchSignals {
            domain: CacheDomain::GatewayRoutes,
            epochs: epochs.clone(),
            version: Arc::new(Version::default()),
            poll: Duration::from_secs(3600),
        },
        move || {
            let reloaded = reloaded.clone();
            async move { Ok::<_, std::convert::Infallible>(reloaded) }
        },
    );
    let kernel = resolve(
        seed(&ctx, services.clone()),
        vec![
            Probe::new("fx-late", |c| {
                c.secret(SecretName::new(LATE));
                c.provide(|r: &Resolved<'_>| -> Result<LateSecrets, ProvideError> {
                    Ok(LateSecrets(r.secrets()?))
                });
            })
            .granted(&[LATE]),
        ],
    )
    .expect("the name is unreferenced at boot");
    let secrets = kernel.state().get::<LateSecrets>().expect("held").0.clone();
    assert_eq!(
        secrets
            .secret(LATE)
            .map(|s| s.expose().to_owned())
            .as_deref(),
        Some("late")
    );

    for _ in 0..200 {
        if services.config.reloads() > 0 {
            break;
        }
        epochs.advance(CacheDomain::GatewayRoutes);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(
        services.config.reloads() > 0,
        "the services config reloaded"
    );
    assert!(
        secrets.secret(LATE).is_none(),
        "a name the reloaded config references is refused to the install's held view"
    );
}

#[derive(Clone)]
struct LateSecrets(systemprompt_traits::DeclaredSecrets);

#[tokio::test]
async fn a_route_sets_resolution_reads_no_declared_secrets() {
    let ctx = context().await;
    let kernel = resolve(
        seed(&ctx, reference_services(&ctx)),
        vec![granted_declaring("fx-install", &[OWN])],
    )
    .expect("admitted");
    let owner = systemprompt_identifiers::ExtensionId::new("fx-install");
    assert!(
        kernel
            .route_scoped(&owner, systemprompt_extension::StandingTag::Install, &[])
            .secrets()
            .is_err()
    );
}
