//! `config apply` end to end over core's own planes: the governance chain and
//! gateway policies declared in a services tree reach their tables, a second
//! apply writes nothing, and the engine built from the rows is the declared
//! chain.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::prelude::{ChangeActor, ConfigPlane};
use systemprompt_gateway::GatewayPoliciesPlane;
use systemprompt_governance::policy::{GovernanceEngine, GovernancePlane};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::ConfigPlaneSet;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{ApplyOptions, ConfigApplyService, ConfigReconcileService};
use systemprompt_test_fixtures::DisposableDb;

fn write(root: &std::path::Path, file: &str, body: &str) {
    let path = root.join(file);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, body).expect("write");
}

#[tokio::test]
async fn config_apply_writes_governance_and_gateway_policies_once() {
    let db = DisposableDb::with_schema("coreplanes").await;
    let pool = db.test_pool().await;
    let root = tempfile::tempdir().expect("tempdir");
    write(
        root.path(),
        "governance/config.yaml",
        "governance:\n  mode: warn\n  policies:\n    - id: scope_check\n      mode: enforce\n",
    );
    write(
        root.path(),
        "gateway/policies.yaml",
        "policies:\n  - name: base\n    priority: 1\n",
    );
    let planes: Vec<Arc<dyn ConfigPlane>> =
        vec![Arc::new(GovernancePlane), Arc::new(GatewayPoliciesPlane)];
    let set =
        ConfigPlaneSet::new(planes, Default::default(), root.path().to_path_buf()).expect("set");
    let service = ConfigApplyService::new(ConfigReconcileService::new(
        set,
        ConfigReconcileRepository::new(&pool),
        Dependencies::new(),
    ));
    let options = ApplyOptions {
        planes: &[],
        actor: ChangeActor::Cli,
        reason: "deploy",
        dry_run: false,
        wait: Duration::from_secs(30),
    };

    let first = service.apply(&options).await.expect("first apply");
    assert_eq!(first.applied(), 3, "{first:?}");
    let second = service.apply(&options).await.expect("second apply");
    assert_eq!(second.applied(), 0, "{second:?}");

    let engine = GovernanceEngine::load(pool.pool().as_ref(), None)
        .await
        .expect("engine");
    let (cfg, _) = engine.policies().next().expect("first policy");
    assert_eq!(cfg.id.as_str(), "scope_check");
    assert!(!cfg.mode.is_warn());
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM ai_gateway_policies")
        .fetch_all(&*pool.pool())
        .await
        .expect("policies");
    assert_eq!(names, ["base"]);
    db.drop_now().await;
}

#[tokio::test]
async fn the_governance_engine_is_unapplied_until_config_apply_runs() {
    let db = DisposableDb::with_schema("coreplanesempty").await;
    let pool = db.test_pool().await;
    let root = tempfile::tempdir().expect("tempdir");
    let planes: Vec<Arc<dyn ConfigPlane>> =
        vec![Arc::new(GovernancePlane), Arc::new(GatewayPoliciesPlane)];
    let set =
        ConfigPlaneSet::new(planes, Default::default(), root.path().to_path_buf()).expect("set");
    let service = ConfigApplyService::new(ConfigReconcileService::new(
        set,
        ConfigReconcileRepository::new(&pool),
        Dependencies::new(),
    ));
    let raw = pool.pool();
    let load = || {
        GovernanceEngine::load(
            raw.as_ref(),
            Some(systemprompt_test_fixtures::offline_rate_windows()),
        )
    };

    assert!(!load().await.expect("engine").is_applied());

    let options = ApplyOptions {
        planes: &[],
        actor: ChangeActor::Cli,
        reason: "deploy",
        dry_run: false,
        wait: Duration::from_secs(30),
    };
    service.apply(&options).await.expect("apply");

    assert!(
        load().await.expect("engine").is_applied(),
        "an applied plane that declares no chain is applied, not denied"
    );
    db.drop_now().await;
}
