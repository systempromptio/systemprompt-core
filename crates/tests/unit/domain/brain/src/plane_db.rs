//! DB-backed: the `brain` plane applied, read back and exported against the
//! test database inside a transaction that is rolled back. Interests are
//! left out because their owner must be an existing user.

use systemprompt_brain::plane::{BrainPlane, current_pipeline_version, load_declared_state};
use systemprompt_extension::prelude::{
    ConfigPlane, PlaneContext, PlaneTx, ResolvedEntities, ResolvedEntity,
};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::Dependencies;

use crate::plane::declare_without_interest;

type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

#[tokio::test]
async fn applied_declarations_read_back_and_export() {
    let pool = test_db_pool().await;
    let root = tempfile::tempdir().expect("tempdir");
    let tag = uuid::Uuid::new_v4().simple().to_string()[..10].to_owned();
    declare_without_interest(root.path(), &tag);
    let declared = BrainPlane
        .sources(root.path())
        .expect("sources")
        .expect("declared");
    let resolved: ResolvedEntities = declared
        .iter()
        .map(|(key, entity)| {
            (
                key.clone(),
                ResolvedEntity {
                    value: Some(entity.value.clone()),
                    source: entity.source.clone(),
                },
            )
        })
        .collect();

    let deps = Dependencies::new();
    let ctx = PlaneContext::new(root.path(), &deps);
    let mut tx: Tx = pool.write_pool().begin().await.expect("begin");
    let outcome = BrainPlane
        .write(&ctx, &mut PlaneTx::new(&mut tx), &resolved)
        .await
        .expect("write");
    assert_eq!(outcome.updated, declared.len());

    let current = BrainPlane
        .read_current(&ctx, &mut PlaneTx::new(&mut tx))
        .await
        .expect("read");
    for (key, entity) in &declared {
        assert_eq!(current.get(key), Some(&entity.value), "{key}");
    }

    let exported = BrainPlane
        .export(&ctx, &mut PlaneTx::new(&mut tx))
        .await
        .expect("export")
        .expect("rows");
    assert!(exported.contains(&format!("kinds/owner-{tag}.yaml")));

    let typed = load_declared_state(&mut tx).await.expect("typed");
    assert!(
        typed
            .kinds
            .iter()
            .any(|k| k.kind.as_str() == format!("owner-{tag}"))
    );
    let first = current_pipeline_version(&mut tx, Default::default())
        .await
        .expect("version");
    let again = current_pipeline_version(&mut tx, Default::default())
        .await
        .expect("version");
    assert_eq!(first, again);
    tx.rollback().await.expect("rollback");
}
