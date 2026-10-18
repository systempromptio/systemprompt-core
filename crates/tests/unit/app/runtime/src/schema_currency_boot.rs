//! `assert_schema_current` and `recheck` against the live test database: a
//! behind schema refuses the boot with the pending work, an ahead additive
//! schema is served and reported, an ahead `@compat: breaking` schema refuses
//! the boot, and a running node's state follows ledger rows a newer release
//! writes after boot.

use std::sync::Arc;

use systemprompt_config::ProfileBootstrap;
use systemprompt_database::{Database, MigrationService, SchemaInstallReport};
use systemprompt_extension::{
    Extension, ExtensionMetadata, ExtensionRegistry, Migration, SchemaDefinition,
};
use systemprompt_manifest::Profile;
use systemprompt_runtime::schema_currency::{recheck, refuse_breaking};
use systemprompt_runtime::{RuntimeError, SchemaState, assert_schema_current};

use crate::boot::{BootOptions, boot};

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[derive(Clone)]
struct CurrencyExt {
    id: &'static str,
    table: &'static str,
    migrations: Vec<Migration>,
}

impl Extension for CurrencyExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "schema-currency-boot",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            self.table.to_owned(),
            format!(
                "CREATE TABLE IF NOT EXISTS {} (id TEXT PRIMARY KEY);",
                self.table
            ),
        )]
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }
}

struct Fixture {
    db: Arc<Database>,
    id: &'static str,
    table: &'static str,
    _boot: crate::boot::BootFixture,
}

impl Fixture {
    fn create(&self) -> Migration {
        Migration::new(
            1,
            "create",
            leak(format!(
                "CREATE TABLE IF NOT EXISTS {} (id TEXT PRIMARY KEY);",
                self.table
            )),
        )
    }

    fn expand(&self) -> Migration {
        Migration::new(
            2,
            "expand",
            leak(format!(
                "ALTER TABLE {} ADD COLUMN IF NOT EXISTS expanded INT;",
                self.table
            )),
        )
    }

    fn contract(&self) -> Migration {
        Migration::new(
            2,
            "contract",
            leak(format!(
                "-- @compat: breaking\nALTER TABLE {} ADD COLUMN IF NOT EXISTS contracted INT;",
                self.table
            )),
        )
    }

    fn ext(&self, migrations: Vec<Migration>) -> CurrencyExt {
        CurrencyExt {
            id: self.id,
            table: self.table,
            migrations,
        }
    }

    fn registry(&self, migrations: Vec<Migration>) -> ExtensionRegistry {
        let mut registry = ExtensionRegistry::new();
        registry
            .register(Arc::new(self.ext(migrations)))
            .expect("register currency extension");
        registry
    }

    async fn migrate(&self, migrations: Vec<Migration>) {
        MigrationService::new(self.db.write())
            .run_pending_migrations(&self.ext(migrations))
            .await
            .expect("migrations apply");
    }

    fn profile(&self) -> &'static Profile {
        ProfileBootstrap::get().expect("profile bootstrapped")
    }

    async fn cleanup(&self) {
        let pool = self.db.pool();
        let _ = sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
            .bind(self.id)
            .execute(pool.as_ref())
            .await;
        let _ = sqlx::query("DELETE FROM extension_migration_compat WHERE extension_id = $1")
            .bind(self.id)
            .execute(pool.as_ref())
            .await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP TABLE IF EXISTS {}",
            self.table
        )))
        .execute(pool.as_ref())
        .await;
    }
}

async fn fixture() -> Fixture {
    let boot = boot(&BootOptions::default());
    let db = Database::new_postgres(&boot.database_url)
        .await
        .expect("connect to test postgres");
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    Fixture {
        db: Arc::new(db),
        id: leak(format!("currency-boot-{suffix}")),
        table: leak(format!("currency_boot_{suffix}")),
        _boot: boot,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_uninstalled_extension_refuses_the_boot_as_fresh() {
    let f = fixture().await;
    let registry = f.registry(vec![f.create()]);

    let err = assert_schema_current(&registry, f.db.write(), f.profile())
        .await
        .expect_err("a never-installed extension is behind");
    match err {
        RuntimeError::SchemaBehind {
            profile,
            fresh,
            pending,
            drift,
        } => {
            assert_eq!(profile, f.profile().name);
            assert_eq!(fresh, vec![f.id.to_owned()]);
            assert!(pending.is_empty());
            assert!(drift.is_empty());
        },
        other => panic!("expected SchemaBehind, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pending_migration_refuses_the_boot_and_names_it() {
    let f = fixture().await;
    f.migrate(vec![f.create()]).await;
    let registry = f.registry(vec![f.create(), f.expand()]);

    let err = assert_schema_current(&registry, f.db.write(), f.profile())
        .await
        .expect_err("an unapplied migration is behind");
    match err {
        RuntimeError::SchemaBehind { pending, fresh, .. } => {
            assert!(fresh.is_empty());
            assert_eq!(pending, vec![format!("{}:002 expand", f.id)]);
        },
        other => panic!("expected SchemaBehind, got {other:?}"),
    }
    f.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_edited_applied_migration_refuses_the_boot_as_drift() {
    let f = fixture().await;
    f.migrate(vec![f.create()]).await;
    let edited = Migration::new(1, "create", "SELECT 1;");
    let registry = f.registry(vec![edited]);

    let err = assert_schema_current(&registry, f.db.write(), f.profile())
        .await
        .expect_err("a changed checksum is drift");
    match err {
        RuntimeError::SchemaBehind { drift, pending, .. } => {
            assert!(pending.is_empty());
            assert_eq!(drift, vec![format!("{}:001 create", f.id)]);
        },
        other => panic!("expected SchemaBehind, got {other:?}"),
    }
    f.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_current_schema_boots_with_nothing_ahead() {
    let f = fixture().await;
    f.migrate(vec![f.create()]).await;
    let registry = f.registry(vec![f.create()]);

    let ahead = assert_schema_current(&registry, f.db.write(), f.profile())
        .await
        .expect("an up-to-date schema is current");
    assert!(ahead.is_empty());
    f.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ahead_additive_migration_is_served_and_reported() {
    let f = fixture().await;
    f.migrate(vec![f.create(), f.expand()]).await;
    let registry = f.registry(vec![f.create()]);

    let ahead = assert_schema_current(&registry, f.db.write(), f.profile())
        .await
        .expect("an ahead additive schema is serviceable");
    assert_eq!(ahead.len(), 1);
    assert_eq!(ahead[0].extension.as_str(), f.id);
    assert_eq!(ahead[0].version, 2);
    assert_eq!(ahead[0].name, "expand");
    assert!(!ahead[0].breaks_older);
    f.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drift_under_an_ahead_extension_is_left_to_the_newer_release() {
    let f = fixture().await;
    f.migrate(vec![f.create(), f.expand()]).await;
    let registry = f.registry(vec![Migration::new(1, "create", "SELECT 1;")]);

    let ahead = assert_schema_current(&registry, f.db.write(), f.profile())
        .await
        .expect("superseded drift does not fail the boot");
    assert_eq!(ahead.len(), 1);
    f.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ahead_breaking_migration_refuses_the_boot() {
    let f = fixture().await;
    f.migrate(vec![f.create(), f.contract()]).await;
    let registry = f.registry(vec![f.create()]);

    let err = assert_schema_current(&registry, f.db.write(), f.profile())
        .await
        .expect_err("a contracted schema cannot be served by the older binary");
    match err {
        RuntimeError::SchemaBreaksThisRelease {
            profile,
            migrations,
        } => {
            assert_eq!(profile, f.profile().name);
            assert_eq!(migrations, vec![format!("{}:002 contract", f.id)]);
        },
        other => panic!("expected SchemaBreaksThisRelease, got {other:?}"),
    }
    f.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refuse_breaking_passes_an_additive_ahead_set() {
    let f = fixture().await;
    f.migrate(vec![f.create(), f.expand()]).await;
    let registry = f.registry(vec![f.create()]);
    let ahead = assert_schema_current(&registry, f.db.write(), f.profile())
        .await
        .expect("additive ahead");

    refuse_breaking(&ahead, f.profile()).expect("nothing in the set is breaking");
    refuse_breaking(&[], f.profile()).expect("an empty set is never breaking");
    f.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recheck_picks_up_a_migration_applied_after_boot() {
    let f = fixture().await;
    f.migrate(vec![f.create()]).await;
    let registry = f.registry(vec![f.create()]);
    let state = SchemaState::from(SchemaInstallReport::default());

    recheck(&registry, f.db.write(), &state)
        .await
        .expect("recheck of a current schema");
    assert!(state.ahead().is_empty());

    f.migrate(vec![f.create(), f.expand()]).await;
    recheck(&registry, f.db.write(), &state)
        .await
        .expect("recheck sees the newer migration");
    let ahead = state.ahead();
    assert_eq!(ahead.len(), 1);
    assert_eq!(ahead[0].version, 2);
    assert!(!state.breaks_this_release());

    recheck(&registry, f.db.write(), &state)
        .await
        .expect("an unchanged ahead set rechecks quietly");
    assert_eq!(state.ahead(), ahead);
    f.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recheck_marks_the_node_unready_on_a_contract_migration() {
    let f = fixture().await;
    f.migrate(vec![f.create()]).await;
    let registry = f.registry(vec![f.create()]);
    let state = SchemaState::from(SchemaInstallReport::default());

    f.migrate(vec![f.create(), f.contract()]).await;
    recheck(&registry, f.db.write(), &state)
        .await
        .expect("recheck sees the contract migration");
    assert!(state.breaks_this_release());
    assert!(
        !state.boot().is_ahead(),
        "the boot report stays what the node booted with"
    );
    f.cleanup().await;
}
