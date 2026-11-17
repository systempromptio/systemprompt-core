//! The install's table-ownership check counts a member held back because the
//! database is ahead of it: a member referencing the held-back member's table
//! as a cross-extension table installs, as it does when neither is ahead.

use systemprompt_database::{MigrationConfig, install_schema_members, schema_members};
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, Migration, SchemaDefinition,
};
use systemprompt_test_fixtures::{DisposableDb, fixture_admitted_of};

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

struct Owner {
    id: &'static str,
    table: &'static str,
}

impl Extension for Owner {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "held-back-owner",
            version: "0.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        let create = leak(format!(
            "CREATE TABLE IF NOT EXISTS {} (id TEXT PRIMARY KEY);",
            self.table
        ));
        c.schema(SchemaDefinition::new(self.table, create))
            .migrations(vec![Migration::new(1, "create", create)]);
        Ok(())
    }
}

struct Referrer {
    id: &'static str,
    table: &'static str,
    cross: &'static str,
}

impl Extension for Referrer {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "held-back-referrer",
            version: "0.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.schema(SchemaDefinition::new(
            self.table,
            format!("CREATE TABLE IF NOT EXISTS {} (id TEXT PRIMARY KEY);", self.table),
        ));
        c.cross_table(self.cross);
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_held_back_member_still_owns_its_tables_for_a_cross_extension_reference() {
    let disposable = DisposableDb::with_schema("held_back_owner").await;
    let db = disposable.test_pool().await;
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let owner_id = leak(format!("held-owner-{suffix}"));
    let owned = leak(format!("held_owned_{suffix}"));
    let admitted = fixture_admitted_of(vec![
        Box::new(Owner {
            id: owner_id,
            table: owned,
        }),
        Box::new(Referrer {
            id: leak(format!("held-referrer-{suffix}")),
            table: leak(format!("held_referrer_{suffix}")),
            cross: owned,
        }),
    ]);
    let members = schema_members(admitted.built());

    let first = install_schema_members(&members, db.write(), MigrationConfig::default()).await;
    assert!(first.is_ok(), "control: neither member ahead installs: {first:?}");

    sqlx::query(
        "INSERT INTO extension_migrations (id, extension_id, version, name, checksum) \
         VALUES ($1, $2, 2, 'newer', 'newer')",
    )
    .bind(format!("{owner_id}_002"))
    .bind(owner_id)
    .execute(db.pool().as_ref())
    .await
    .expect("record a migration newer than the owner declares");

    let ahead = install_schema_members(&members, db.write(), MigrationConfig::default()).await;
    let report = ahead.unwrap_or_else(|e| panic!("the held-back owner still owns {owned}: {e:?}"));
    assert!(
        report.ahead.iter().any(|a| a.extension.as_str() == owner_id),
        "the owner is reported ahead, so it was held back: {report:?}"
    );

    drop(db);
    disposable.drop_now().await;
}
