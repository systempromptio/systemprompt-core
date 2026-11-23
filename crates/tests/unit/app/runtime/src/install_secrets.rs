//! An install reads only the secrets it declares (`Contributions::secret`),
//! never the custom namespace: a declared name a core member, the profile or
//! the platform references fails admission typed, an undeclared name reads
//! nothing, and a name a services reload starts to reference is refused from
//! then on.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::{FactoryError, ProvideError, Resolved};
use systemprompt_identifiers::SecretName;
use systemprompt_manifest::services::{ServicesConfig, ServicesView, SlackAppConfig};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{AppContext, Kernel, KernelError};
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, probe_admitted, test_database_url,
};
use systemprompt_traits::{
    BackgroundTasks, CacheDomain, CacheEpochs, ConfigVersionSource, ConfigWatch, ExtensionSecret,
    ReloadOutcome, RepositoryError, SECRET_SEED_BYTES, SecretReferences, SecretStore,
    SecretStoreError, SecretsStatus, WatchSignals,
};

const SLACK_SIGNING_REF: &str = "slack_signing_secret";
const OWN: &str = "fx_install_secret";
const LATE: &str = "fx_late_secret";

#[derive(Debug)]
struct Store(BTreeMap<&'static str, &'static str>);

#[async_trait::async_trait]
impl SecretStore for Store {
    fn secret(&self, name: &str) -> Option<ExtensionSecret> {
        self.0
            .get(name)
            .map(|v| ExtensionSecret::new((*v).to_owned()))
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

fn seed(ctx: &AppContext, services: ServicesView) -> Dependencies {
    let mut state = ctx.kernel().state().clone();
    state.insert(services);
    state.insert_sealed(store());
    state
}

fn reference_services(ctx: &AppContext) -> ServicesView {
    let base = ctx
        .kernel()
        .state()
        .get::<ServicesView>()
        .expect("the services view is a kernel seed")
        .clone();
    ServicesView {
        config: ConfigWatch::new(with_slack(&base.config(), SLACK_SIGNING_REF)),
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
        vec![declaring("fx-install", &[OWN])],
    )
    .expect("an install declaring only its own secret is admitted");
    let read = kernel.state().get::<Read>().expect("the install's value");
    assert_eq!(read.own.as_deref(), Some("mine"));
    assert_eq!(
        read.slack, None,
        "the Slack signing secret is the slack member's, never an install's read"
    );
    assert_eq!(read.late, None, "an undeclared name reads nothing");
}

fn refused(error: &KernelError, owner: &str, name: &str) -> bool {
    matches!(
        error,
        KernelError::ReservedSecret { owner: o, name: n } if o.as_str() == owner && n.as_str() == name
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
    ] {
        let error = resolve(
            seed(&ctx, reference_services(&ctx)),
            vec![declaring("fx-install", &[name])],
        )
        .expect_err("a core-referenced name is refused at admission");
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
        vec![reserving, declaring("fx-install", &[OWN])],
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
        vec![Probe::new("fx-late", |c| {
            c.secret(SecretName::new(LATE));
            c.provide(|r: &Resolved<'_>| -> Result<LateSecrets, ProvideError> {
                Ok(LateSecrets(r.secrets()?))
            });
        })],
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
        vec![declaring("fx-install", &[OWN])],
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
