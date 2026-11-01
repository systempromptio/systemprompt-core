use systemprompt_database::{DatabaseProvider, MigrationService, PostgresProvider};
use systemprompt_extension::{
    Extension, ExtensionMetadata, LoaderError, Migration, SchemaDefinition,
};

use crate::services::db_helper::test_pool;

struct Ext {
    id: &'static str,
    migrations: Vec<Migration>,
}

impl Extension for Ext {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "repair",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }
}

async fn provider() -> PostgresProvider {
    PostgresProvider::from_pool(test_pool().await.write_pool())
}

async fn reset(provider: &PostgresProvider, ext: &str, table: &str) {
    provider
        .execute_raw(&format!(
            "DELETE FROM extension_migrations WHERE extension_id = '{ext}'"
        ))
        .await
        .expect("clear tracking");
    provider
        .execute_raw(&format!("DROP TABLE IF EXISTS {table}"))
        .await
        .expect("drop table");
}

async fn column_exists(provider: &PostgresProvider, table: &str, column: &str) -> bool {
    let row: (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = $1 AND \
         column_name = $2)",
    )
    .bind(table)
    .bind(column)
    .fetch_one(&*provider.get_postgres_pool())
    .await
    .expect("information_schema");
    row.0
}

#[tokio::test]
async fn a_drifted_no_transaction_migration_is_reapplied_and_restamped() {
    let provider = provider().await;
    reset(&provider, "repair_nt_ext", "repair_nt_t").await;
    let service = MigrationService::new(&provider);

    let original = Ext {
        id: "repair_nt_ext",
        migrations: vec![Migration::new_no_transaction(
            1,
            "nt",
            "CREATE TABLE IF NOT EXISTS repair_nt_t (id BIGINT PRIMARY KEY);",
        )],
    };
    service
        .run_pending_migrations(&original)
        .await
        .expect("apply");

    let edited = Ext {
        id: "repair_nt_ext",
        migrations: vec![Migration::new_no_transaction(
            1,
            "nt",
            "CREATE TABLE IF NOT EXISTS repair_nt_t (id BIGINT PRIMARY KEY); ALTER TABLE \
             repair_nt_t ADD COLUMN IF NOT EXISTS note TEXT;",
        )],
    };
    assert_eq!(
        service.status(&edited).await.expect("status").drift.len(),
        1
    );

    let result = service.repair_drift(&edited).await.expect("repair");
    assert_eq!(result.reapplied, 1);
    assert_eq!(result.repaired.len(), 1);
    assert_eq!(result.migrations_run, 0);
    assert!(column_exists(&provider, "repair_nt_t", "note").await);
    assert!(
        service
            .status(&edited)
            .await
            .expect("status")
            .drift
            .is_empty()
    );

    let again = service.repair_drift(&edited).await.expect("repair noop");
    assert_eq!(again.reapplied, 0);
    assert!(again.repaired.is_empty());
}

#[tokio::test]
async fn a_failing_no_transaction_reapply_keeps_the_drift_tracked() {
    let provider = provider().await;
    reset(&provider, "repair_ntfail_ext", "repair_ntfail_t").await;
    let service = MigrationService::new(&provider);

    let original = Ext {
        id: "repair_ntfail_ext",
        migrations: vec![Migration::new_no_transaction(
            1,
            "ntfail",
            "CREATE TABLE IF NOT EXISTS repair_ntfail_t (id BIGINT PRIMARY KEY);",
        )],
    };
    service
        .run_pending_migrations(&original)
        .await
        .expect("apply");

    let broken = Ext {
        id: "repair_ntfail_ext",
        migrations: vec![Migration::new_no_transaction(
            1,
            "ntfail",
            "CREATE TABLE IF NOT EXISTS repair_ntfail_t (id BIGINT PRIMARY KEY); ALTER TABLE \
             repair_ntfail_t ADD COLUMN id BIGINT;",
        )],
    };
    let err = service
        .repair_drift(&broken)
        .await
        .expect_err("reapply fails");
    match &err {
        LoaderError::MigrationStepFailed { context, .. } => {
            assert!(
                context.contains("re-apply drifted migration 1"),
                "{context}"
            );
        },
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(
        service.status(&broken).await.expect("status").drift.len(),
        1
    );
}

#[tokio::test]
async fn a_failing_transactional_reapply_rolls_back_and_stays_drifted() {
    let provider = provider().await;
    reset(&provider, "repair_tx_ext", "repair_tx_t").await;
    let service = MigrationService::new(&provider);

    let original = Ext {
        id: "repair_tx_ext",
        migrations: vec![Migration::new(
            1,
            "tx",
            "CREATE TABLE IF NOT EXISTS repair_tx_t (id BIGINT PRIMARY KEY);",
        )],
    };
    service
        .run_pending_migrations(&original)
        .await
        .expect("apply");

    let broken = Ext {
        id: "repair_tx_ext",
        migrations: vec![Migration::new(
            1,
            "tx",
            "CREATE TABLE IF NOT EXISTS repair_tx_t (id BIGINT PRIMARY KEY); ALTER TABLE \
             repair_tx_t ADD COLUMN note TEXT; ALTER TABLE repair_tx_t ADD COLUMN id BIGINT;",
        )],
    };
    service
        .repair_drift(&broken)
        .await
        .expect_err("reapply fails");
    assert!(!column_exists(&provider, "repair_tx_t", "note").await);
    assert_eq!(
        service.status(&broken).await.expect("status").drift.len(),
        1
    );

    let reconciled = service.reconcile_drift(&broken).await.expect("reconcile");
    assert_eq!(reconciled.repaired.len(), 1);
    assert_eq!(reconciled.reapplied, 0);
    assert!(!column_exists(&provider, "repair_tx_t", "note").await);
    assert!(
        service
            .status(&broken)
            .await
            .expect("status")
            .drift
            .is_empty()
    );
}

#[tokio::test]
async fn reconcile_without_drift_is_a_noop() {
    let provider = provider().await;
    reset(&provider, "repair_clean_ext", "repair_clean_t").await;
    let service = MigrationService::new(&provider);
    let ext = Ext {
        id: "repair_clean_ext",
        migrations: vec![Migration::new(
            1,
            "clean",
            "CREATE TABLE IF NOT EXISTS repair_clean_t (id BIGINT PRIMARY KEY);",
        )],
    };
    service.run_pending_migrations(&ext).await.expect("apply");
    let result = service.reconcile_drift(&ext).await.expect("reconcile");
    assert!(result.repaired.is_empty());
    assert_eq!(result.migrations_run, 0);
}
