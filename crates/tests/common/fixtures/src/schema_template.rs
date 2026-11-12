//! The installed-schema template every [`crate::DisposableDb::with_schema`]
//! database is copied from.
//!
//! A full install over every registered extension is the slow part of a
//! schema-backed test, and its result depends only on the binary: the
//! extensions' schemas, migrations, retirements and seeds, and the installer
//! itself. So it runs once per server and fingerprint into a template database,
//! and each test takes a `CREATE DATABASE ... TEMPLATE` copy of it.
//!
//! Tests run as separate processes against one server, so coordination is a
//! Postgres advisory lock on the base database: copiers hold it shared, the one
//! process that builds or retires a template holds it exclusive. A template is
//! built under a staging name (owned by its builder's PID, so the orphan sweep
//! drops one a crashed builder left), closed to connections, and renamed into
//! place as the final step: a database under the template name is a complete
//! install by construction, and nothing ever connects to it.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use sqlx::{Connection, PgConnection, PgPool};
use systemprompt_extension::ExtensionRegistry;

use crate::db::connect;

pub(crate) const TEMPLATE_PREFIX: &str = "sptpl_";
pub(crate) const FINGERPRINT_HEX: usize = 16;
const STAGING_PREFIX: &str = "sptplbuild";
const TEMPLATE_LOCK_KEY: i64 = 0x7370_7470_6c5f_0001;

const INVALID_CATALOG_NAME: &str = "3D000";
const DUPLICATE_DATABASE: &str = "42P04";

pub(crate) async fn create_from_template(admin: &PgPool, base_url: &str, name: &str) -> Result<()> {
    let registry = ExtensionRegistry::discover().context("extension registry discovery")?;
    let template = template_name(&registry)?;
    let mut conn = admin
        .acquire()
        .await
        .context("template lock connection")?
        .detach();

    advisory(&mut conn, "pg_advisory_lock_shared").await?;
    let shared = if exists(&mut conn, &template).await? {
        Some(copy(&mut conn, &template, name).await)
    } else {
        None
    };
    advisory(&mut conn, "pg_advisory_unlock_shared").await?;
    match shared {
        Some(Ok(())) => return close(conn).await,
        Some(Err(e)) if !is_code(&e, INVALID_CATALOG_NAME) => {
            return Err(e).with_context(|| format!("failed to copy {template} into {name}"));
        },
        _ => {},
    }

    advisory(&mut conn, "pg_advisory_lock").await?;
    let built = async {
        if !exists(&mut conn, &template).await? {
            build(&mut conn, &registry, base_url, &template).await?;
        }
        copy(&mut conn, &template, name)
            .await
            .with_context(|| format!("failed to copy {template} into {name}"))
    }
    .await;
    advisory(&mut conn, "pg_advisory_unlock").await?;
    built?;
    close(conn).await
}

async fn build(
    conn: &mut PgConnection,
    registry: &ExtensionRegistry,
    base_url: &str,
    template: &str,
) -> Result<()> {
    crate::orphans::sweep_stale_templates(conn, template).await;
    let staging = crate::orphans::database_name(STAGING_PREFIX);
    execute(conn, &format!("CREATE DATABASE \"{staging}\"")).await?;

    let installed = install(registry, &sibling_url(base_url, &staging)?).await;
    if let Err(e) = installed {
        drop_database(conn, &staging).await;
        return Err(e);
    }

    execute(
        conn,
        &format!("ALTER DATABASE \"{staging}\" WITH ALLOW_CONNECTIONS false"),
    )
    .await?;
    let built_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock before the Unix epoch")?
        .as_secs();
    execute(
        conn,
        &format!("COMMENT ON DATABASE \"{staging}\" IS '{built_at}'"),
    )
    .await?;
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
        .bind(&staging)
        .execute(&mut *conn)
        .await
        .context("failed to close the staging database's sessions")?;
    match execute(
        conn,
        &format!("ALTER DATABASE \"{staging}\" RENAME TO \"{template}\""),
    )
    .await
    {
        Err(e) if is_code(&e, DUPLICATE_DATABASE) => {
            drop_database(conn, &staging).await;
            Ok(())
        },
        other => other.with_context(|| format!("failed to publish the template {template}")),
    }
}

// Why: the schema is installed through the same entry point the server
// boots with, so the database a test starts from is the shape a real
// fresh install produces -- baseline stamps included.
async fn install(registry: &ExtensionRegistry, url: &str) -> Result<()> {
    let pool = connect(url).await?;
    let installed = systemprompt_database::install_extension_schemas_full(
        registry,
        pool.write(),
        &[],
        systemprompt_database::MigrationConfig::default(),
    )
    .await
    .context("failed to install the extension schemas into the template database");
    pool.pool().close().await;
    pool.write_pool().close().await;
    installed.map(|_| ())
}

fn template_name(registry: &ExtensionRegistry) -> Result<String> {
    let extensions = registry
        .enabled_schema_extensions(&[])
        .context("schema extensions for the template fingerprint")?;
    let mut hasher = Sha256::new();
    let mut field = |bytes: &[u8]| {
        hasher.update(bytes);
        hasher.update([0]);
    };
    field(env!("SYSTEMPROMPT_INSTALLER_SOURCE_DIGEST").as_bytes());
    for ext in &extensions {
        field(ext.id().as_bytes());
        field(ext.version().as_bytes());
        for schema in ext.schemas() {
            field(schema.table.as_deref().unwrap_or_default().as_bytes());
            field(schema.schema_name().as_bytes());
            field(schema.sql.as_bytes());
            field(schema.required_columns.join(",").as_bytes());
        }
        for migration in ext.migrations() {
            field(&migration.version.to_le_bytes());
            field(migration.name.as_bytes());
            field(migration.checksum().as_bytes());
            field(&[u8::from(migration.no_transaction)]);
        }
        for retirement in ext.retirements() {
            field(retirement.sql.as_bytes());
        }
        for table in ext.cross_extension_tables() {
            field(table.as_bytes());
        }
        for seed in ext.seeds() {
            field(seed.id.as_bytes());
            field(seed.sql.as_bytes());
        }
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{TEMPLATE_PREFIX}{}", &hex[..FINGERPRINT_HEX]))
}

pub(crate) fn sibling_url(base_url: &str, name: &str) -> Result<String> {
    let (base, _old) = base_url
        .rsplit_once('/')
        .context("DATABASE_URL must name a database")?;
    Ok(format!("{base}/{name}"))
}

async fn exists(conn: &mut PgConnection, name: &str) -> Result<bool> {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
        .bind(name)
        .fetch_one(&mut *conn)
        .await
        .context("failed to look up the template database")
}

async fn copy(conn: &mut PgConnection, template: &str, name: &str) -> Result<()> {
    execute(
        conn,
        &format!("CREATE DATABASE \"{name}\" TEMPLATE \"{template}\""),
    )
    .await
}

async fn advisory(conn: &mut PgConnection, function: &str) -> Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(format!("SELECT {function}($1)")))
        .bind(TEMPLATE_LOCK_KEY)
        .execute(&mut *conn)
        .await
        .with_context(|| format!("{function} on the template lock"))
        .map(|_| ())
}

async fn close(conn: PgConnection) -> Result<()> {
    conn.close()
        .await
        .context("failed to close the template lock connection")
}

async fn execute(conn: &mut PgConnection, sql: &str) -> Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(&mut *conn)
        .await
        .map(|_| ())
        .map_err(anyhow::Error::from)
}

async fn drop_database(conn: &mut PgConnection, name: &str) {
    if let Err(e) = execute(
        conn,
        &format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"),
    )
    .await
    {
        eprintln!("LEAKED staging template database {name}: {e:#}");
    }
}

fn is_code(error: &anyhow::Error, code: &str) -> bool {
    error
        .downcast_ref::<sqlx::Error>()
        .and_then(sqlx::Error::as_database_error)
        .and_then(|db| db.code())
        .is_some_and(|c| c == code)
}
