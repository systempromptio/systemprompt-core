//! End-to-end `AppContextBuilder::build` runs against a full tempdir profile
//! and the live test database. Each test bootstraps its own process-global
//! profile/secrets/config (nextest is process-per-test), seeds or omits the
//! configured system-admin row, and asserts either the assembled context or
//! the exact `RuntimeError` variant.

use systemprompt_runtime::{AppContext, RuntimeError};

use crate::boot::{BootOptions, boot};

fn unique_admin(tag: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .subsec_nanos();
    format!("covrt_{tag}_{}_{nanos}", std::process::id())
}

async fn seed_admin(url: &str, name: &str, status: &str, roles: &[&str]) -> sqlx::PgPool {
    let pool = sqlx::PgPool::connect(url).await.expect("connect test db");
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    sqlx::query("INSERT INTO users (id, name, email, status, roles) VALUES ($1, $2, $3, $4, $5)")
        .bind(format!("usr_{name}"))
        .bind(name)
        .bind(format!("{name}@example.test"))
        .bind(status)
        .bind(&roles)
        .execute(&pool)
        .await
        .expect("insert admin fixture user");
    pool
}

async fn remove_admin(pool: &sqlx::PgPool, name: &str) {
    sqlx::query("DELETE FROM users WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await
        .expect("remove admin fixture user");
}

#[tokio::test]
async fn build_assembles_full_context_with_pool_and_write_url() {
    let admin = unique_admin("ok");
    let Some(fixture) = boot(&BootOptions {
        admin_username: admin.clone(),
        pool_settings: true,
        write_url: true,
        ..BootOptions::default()
    }) else {
        return;
    };
    let pool = seed_admin(&fixture.database_url, &admin, "active", &["admin", "user"]).await;

    let ctx = AppContext::builder()
        .with_extensions(systemprompt_extension::ExtensionRegistry::new())
        .with_migrations(true)
        .build()
        .await
        .expect("full build must succeed with a seeded admin");

    assert_eq!(ctx.system_admin().username(), admin);
    assert_eq!(ctx.config().system_admin_username, admin);
    assert_eq!(ctx.server_address(), "127.0.0.1:8080");
    assert!(
        ctx.config().database_write_url.is_some(),
        "write url from secrets must be threaded into the config"
    );
    assert!(
        ctx.content_config().is_some(),
        "the fixture services/content/config.yaml must be loaded"
    );
    assert!(
        ctx.content_routing().is_some(),
        "a loaded content config provides routing"
    );
    assert!(ctx.geoip_reader().is_none());
    assert_eq!(ctx.extension_registry().ids().len(), 0);
    assert!(
        format!("{:?}", ctx.marketplace_filter()).contains("AllowAllFilter"),
        "no inventory filter factory is registered, so the allow-all fallback applies; got {:?}",
        ctx.marketplace_filter()
    );

    remove_admin(&pool, &admin).await;
}

#[tokio::test]
async fn build_fails_when_system_admin_is_missing() {
    let admin = unique_admin("miss");
    let Some(_fixture) = boot(&BootOptions {
        admin_username: admin.clone(),
        ..BootOptions::default()
    }) else {
        return;
    };

    let err = AppContext::builder()
        .with_extensions(systemprompt_extension::ExtensionRegistry::new())
        .build()
        .await
        .expect_err("build must fail without the admin row");
    match err {
        RuntimeError::SystemAdminNotFound { username } => assert_eq!(username, admin),
        other => panic!("expected SystemAdminNotFound, got: {other:?}"),
    }
}

#[tokio::test]
async fn build_fails_when_system_admin_is_inactive() {
    let admin = unique_admin("inact");
    let Some(fixture) = boot(&BootOptions {
        admin_username: admin.clone(),
        ..BootOptions::default()
    }) else {
        return;
    };
    let pool = seed_admin(&fixture.database_url, &admin, "inactive", &["admin"]).await;

    let err = AppContext::builder()
        .with_extensions(systemprompt_extension::ExtensionRegistry::new())
        .build()
        .await
        .expect_err("build must fail with an inactive admin");
    match err {
        RuntimeError::SystemAdminInactive { username } => assert_eq!(username, admin),
        other => panic!("expected SystemAdminInactive, got: {other:?}"),
    }

    remove_admin(&pool, &admin).await;
}

#[tokio::test]
async fn build_fails_when_system_admin_lacks_admin_role() {
    let admin = unique_admin("role");
    let Some(fixture) = boot(&BootOptions {
        admin_username: admin.clone(),
        ..BootOptions::default()
    }) else {
        return;
    };
    let pool = seed_admin(&fixture.database_url, &admin, "active", &["user"]).await;

    let err = AppContext::builder()
        .with_extensions(systemprompt_extension::ExtensionRegistry::new())
        .build()
        .await
        .expect_err("build must fail when the admin role is missing");
    match err {
        RuntimeError::SystemAdminMissingRole { username } => assert_eq!(username, admin),
        other => panic!("expected SystemAdminMissingRole, got: {other:?}"),
    }

    remove_admin(&pool, &admin).await;
}
