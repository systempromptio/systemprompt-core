//! An owner reassignment is admitted against the catalog, not against the
//! member's declared text: a reassignment over a security table is refused at
//! install whatever the member's schema declares, a reassignment naming a
//! column the live table lacks is refused, and one over a table recorded to
//! another member is refused.

use systemprompt_database::DatabaseProvider;
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, OwnerReassignment,
    ReassignAction, SchemaDefinition,
};
use systemprompt_test_fixtures::DisposableDb;

use super::members::{admitted_of, install};

struct Reassigning {
    id: &'static str,
    table: &'static str,
    ddl: &'static str,
    column: &'static str,
}

impl Extension for Reassigning {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: self.id,
            version: "0.0.1",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.schema(SchemaDefinition::new(self.table, self.ddl));
        c.owner_reassignment(OwnerReassignment {
            table: self.table,
            column: self.column,
            action: ReassignAction::UPDATE,
        });
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reassignment_over_a_security_table_is_refused_at_install() {
    let database = DisposableDb::empty("g11_security").await;
    let pool = database.test_pool().await;
    let admitted = admitted_of(vec![Box::new(Reassigning {
        id: "g11_rogue",
        table: "oauth_refresh_tokens",
        ddl: "CREATE TABLE IF NOT EXISTS oauth_refresh_tokens (id TEXT PRIMARY KEY, user_id TEXT);",
        column: "user_id",
    })]);

    let error = install(&admitted, pool.write())
        .await
        .expect_err("a security table is never a reassignment target");

    assert!(
        error.to_string().contains("oauth_refresh_tokens"),
        "refusal names the table, got: {error}"
    );
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_declared_column_the_catalog_lacks_is_refused() {
    let database = DisposableDb::empty("g11_column").await;
    let pool = database.test_pool().await;
    pool.write()
        .execute_batch("CREATE TABLE g11_notes (id BIGINT PRIMARY KEY);")
        .await
        .expect("the live table predates the declaration");
    let admitted = admitted_of(vec![Box::new(Reassigning {
        id: "g11_notes_member",
        table: "g11_notes",
        ddl: "CREATE TABLE IF NOT EXISTS g11_notes (id BIGINT PRIMARY KEY, owner_id TEXT);",
        column: "owner_id",
    })]);

    let error = install(&admitted, pool.write())
        .await
        .expect_err("a reassignment column absent from the live table is refused");

    assert!(
        error.to_string().contains("owner_id"),
        "refusal names the column, got: {error}"
    );
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reassignment_over_a_peer_recorded_table_is_refused() {
    let database = DisposableDb::empty("g11_peer").await;
    let pool = database.test_pool().await;
    pool.write()
        .execute_batch(
            "CREATE TABLE g11_peer (id BIGINT PRIMARY KEY, user_id TEXT);
             CREATE TABLE schema_table_owners (table_name TEXT PRIMARY KEY, extension_id TEXT NOT \
             NULL, standing TEXT NOT NULL DEFAULT 'core', recorded_at TIMESTAMPTZ NOT NULL \
             DEFAULT CURRENT_TIMESTAMP);
             INSERT INTO schema_table_owners (table_name, extension_id, standing)
             VALUES ('public.g11_peer', 'g11_owner', 'core');",
        )
        .await
        .expect("a peer's recorded table");
    let admitted = admitted_of(vec![Box::new(Reassigning {
        id: "g11_claimant",
        table: "g11_peer",
        ddl: "CREATE TABLE IF NOT EXISTS g11_peer (id BIGINT PRIMARY KEY, user_id TEXT);",
        column: "user_id",
    })]);

    let error = install(&admitted, pool.write())
        .await
        .expect_err("a peer's table is never this member's reassignment target");

    assert!(
        error.to_string().contains("g11_peer"),
        "refusal names the table, got: {error}"
    );
    drop(pool);
    database.drop_now().await;
}
