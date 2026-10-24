//! The `governance` config plane: the chain as entities, the engine built
//! from `governance_config`, and the watch that rebuilds it on a change.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_database::{DbPool, PlaneTransaction};
use systemprompt_extension::prelude::{
    ChangeSource, ConfigPlane, PlaneContext, PlaneTx, ResolvedEntities, ResolvedEntity,
};
use systemprompt_governance::policy::plane::{config_from_entities, declared_entities};
use systemprompt_governance::policy::{
    GOVERNANCE_FILE, GovernanceConfig, GovernanceConfigError, GovernanceEngine, GovernancePlane,
};
use systemprompt_manifest::EnforcementMode;
use systemprompt_test_fixtures::DisposableDb;
use systemprompt_traits::{
    CacheDomain, CacheEpochs, ConfigVersionSource, ConfigWatch, Dependencies, RepositoryError,
    WatchSignals,
};

const CHAIN: &str = "governance:\n  mode: warn\n  policies:\n    - id: secret_scan\n    - id: scope_check\n      mode: enforce\n";

#[test]
fn the_chain_becomes_one_settings_entity_and_one_entity_per_policy() {
    let entities = declared_entities(CHAIN).expect("entities");
    let keys: Vec<&str> = entities.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["governance", "policy/scope_check", "policy/secret_scan"]
    );
}

#[test]
fn a_duplicate_policy_or_a_bad_mode_is_refused() {
    assert!(matches!(
        declared_entities("governance:\n  policies:\n    - id: a\n    - id: a\n"),
        Err(GovernanceConfigError::DuplicatePolicyId { .. })
    ));
    assert!(matches!(
        declared_entities("governance:\n  mode: warnn\n  policies: []\n"),
        Err(GovernanceConfigError::InvalidMode { .. })
    ));
}

#[test]
fn entities_reassemble_into_the_same_chain_the_file_parses_to() {
    let entities = declared_entities(CHAIN).expect("entities");
    let rebuilt = config_from_entities(&entities)
        .expect("rebuilds")
        .expect("non-empty");
    let parsed = GovernanceConfig::parse(CHAIN).expect("parses");
    let shape = |c: &GovernanceConfig| -> Vec<(String, EnforcementMode)> {
        c.policies
            .iter()
            .map(|p| (p.id.as_str().to_owned(), p.mode))
            .collect()
    };
    assert_eq!(shape(&rebuilt), shape(&parsed));
    assert_eq!(rebuilt.mode, parsed.mode);
    assert!(
        config_from_entities(&Default::default())
            .expect("empty")
            .is_none()
    );
}

async fn write_declared(pool: &DbPool, yaml: &str) {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join(GOVERNANCE_FILE);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, yaml).expect("write");
    let declared = GovernancePlane
        .load_declared(root.path())
        .expect("load")
        .expect("declared");
    let resolved: ResolvedEntities = declared
        .entries
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                ResolvedEntity {
                    value: Some(v),
                    source: ChangeSource::Code(GOVERNANCE_FILE.to_owned()),
                },
            )
        })
        .collect();
    let deps = Dependencies::new();
    let ctx = PlaneContext::new(root.path(), &deps);
    let mut tx: PlaneTransaction = pool.write_pool().begin().await.expect("begin");
    GovernancePlane
        .write(&ctx, &mut PlaneTx::new(&mut tx), &resolved)
        .await
        .expect("write");
    tx.commit().await.expect("commit");
}

fn scope_check_mode(engine: &GovernanceEngine) -> Option<EnforcementMode> {
    engine
        .policies()
        .find(|(c, _)| c.id.as_str() == "scope_check" && c.enabled)
        .map(|(c, _)| c.mode)
}

#[tokio::test]
async fn the_engine_is_unapplied_when_empty_and_follows_the_applied_rows() {
    let db = DisposableDb::with_schema("govplane").await;
    let pool = db.test_pool().await;

    let empty = GovernanceEngine::load(
        pool.pool().as_ref(),
        Some(crate::offline_rate_windows()),
        Default::default(),
    )
    .await
    .expect("unapplied engine");
    assert!(
        !empty.is_applied(),
        "an empty, never-applied plane is unapplied"
    );

    write_declared(&pool, CHAIN).await;
    let applied = GovernanceEngine::load(
        pool.pool().as_ref(),
        Some(crate::offline_rate_windows()),
        Default::default(),
    )
    .await
    .expect("declared chain");
    assert!(applied.is_applied());
    assert_eq!(scope_check_mode(&applied), Some(EnforcementMode::Enforce));
    let source: String =
        sqlx::query_scalar("SELECT source FROM governance_config WHERE entity = 'governance'")
            .fetch_one(&*pool.pool())
            .await
            .expect("source");
    assert_eq!(source, "code:governance/config.yaml");
    db.drop_now().await;
}

struct FixedVersion;

#[async_trait]
impl ConfigVersionSource for FixedVersion {
    async fn current_version(&self) -> Result<i64, RepositoryError> {
        Ok(1)
    }
}

#[tokio::test]
async fn a_governance_epoch_swaps_in_the_rebuilt_engine() {
    let db = DisposableDb::with_schema("govwatch").await;
    let pool = db.test_pool().await;
    let load = |pool: DbPool| async move {
        GovernanceEngine::load(
            pool.pool().as_ref(),
            Some(crate::offline_rate_windows()),
            Default::default(),
        )
        .await
    };
    let watch = ConfigWatch::new(load(Arc::clone(&pool)).await.expect("initial"));
    let epochs = CacheEpochs::new();
    let signals = WatchSignals {
        domain: CacheDomain::Governance,
        epochs: epochs.clone(),
        version: Arc::new(FixedVersion),
        poll: Duration::from_secs(3600),
    };
    let cancel = tokio_util::sync::CancellationToken::new();
    let reload_pool = Arc::clone(&pool);
    let driver = tokio::spawn(watch.clone().run(
        signals,
        move || load(Arc::clone(&reload_pool)),
        cancel.clone(),
    ));
    assert!(!watch.get().is_applied());

    write_declared(&pool, CHAIN).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    epochs.advance(CacheDomain::Governance);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while watch.reloads() == 0 && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(
        scope_check_mode(&watch.get()),
        Some(EnforcementMode::Enforce)
    );
    cancel.cancel();
    driver.await.expect("driver");
    db.drop_now().await;
}
