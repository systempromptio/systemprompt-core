//! The gateway policy resolver over a database whose `gateway_policies` plane
//! was never applied: the default `warn` serves the permissive spec, `deny`
//! refuses with `PolicyUnavailable::NotApplied` in either fault mode, and an
//! applied plane with no policy is permissive under `deny` too.

use systemprompt_ai::repository::AiGatewayPolicyRepository;
use systemprompt_database::DbPool;
use systemprompt_gateway::{PolicyResolver, PolicyUnavailable};
use systemprompt_manifest::profile::UnappliedPlaneMode;
use systemprompt_manifest::services::QuotaFaultMode;
use systemprompt_test_fixtures::{DisposableDb, record_plane_applied};
use systemprompt_traits::CacheEpochs;

fn resolver(pool: &DbPool, mode: UnappliedPlaneMode) -> PolicyResolver {
    PolicyResolver::from_repository(AiGatewayPolicyRepository::new(pool), CacheEpochs::new())
        .with_unapplied_mode(mode)
}

#[tokio::test]
async fn warn_serves_an_unapplied_plane_permissively() {
    let db = DisposableDb::with_schema("gwunappliedwarn").await;
    let pool = db.test_pool().await;

    let spec = resolver(&pool, UnappliedPlaneMode::Warn)
        .resolve(QuotaFaultMode::Closed)
        .await
        .expect("warn passes");
    assert!(spec.quota_windows.is_empty());
    db.drop_now().await;
}

#[tokio::test]
async fn deny_refuses_an_unapplied_plane_in_either_fault_mode() {
    let db = DisposableDb::with_schema("gwunapplieddeny").await;
    let pool = db.test_pool().await;

    for fault in [QuotaFaultMode::Open, QuotaFaultMode::Closed] {
        let refused = resolver(&pool, UnappliedPlaneMode::Deny)
            .resolve(fault)
            .await
            .expect_err("deny refuses");
        assert!(
            matches!(refused, PolicyUnavailable::NotApplied),
            "{refused}"
        );
        assert!(
            refused
                .to_string()
                .contains("run `systemprompt admin config apply`"),
            "{refused}"
        );
    }
    db.drop_now().await;
}

#[tokio::test]
async fn an_applied_empty_plane_is_permissive_under_deny() {
    let db = DisposableDb::with_schema("gwappliedempty").await;
    let pool = db.test_pool().await;
    record_plane_applied(&pool, "gateway_policies")
        .await
        .expect("record the apply");

    resolver(&pool, UnappliedPlaneMode::Deny)
        .resolve(QuotaFaultMode::Closed)
        .await
        .expect("an applied plane serves");
    db.drop_now().await;
}
