//! Regression guards for `systemprompt admin session login` on local profiles.
//!
//! The original bug: `login` unconditionally called
//! `CredentialsBootstrap::require()` to resolve the operator's email, so a
//! local-only checkout (no `~/.systemprompt/cloud/credentials.json`) was locked
//! out of `session login` the moment its session expired. The fix routes local
//! profiles through `resolve_local_admin_email`, which looks the system-admin
//! user up by the `system_admin.username` recorded in the profile.

use systemprompt_cli::admin::session::login::resolve_local_admin_email;
use systemprompt_database::DbPool;
use systemprompt_users::UserService;

async fn get_db() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn resolve_local_admin_email_returns_bootstrapped_user_email() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let service = UserService::new(&db).expect("user service");

    let unique = uuid::Uuid::new_v4();
    let username = format!("login_local_admin_{}", &unique.to_string()[..8]);
    let email = format!("login_local_admin_{}@example.com", unique);

    let created = service
        .create(&username, &email, Some("Local Admin"), None)
        .await
        .expect("create user");
    service
        .assign_roles(&created.id, &["admin".to_owned()])
        .await
        .expect("assign admin role");

    let resolved = resolve_local_admin_email(&username, &db)
        .await
        .expect("local admin email resolves without cloud credentials");

    assert_eq!(resolved, email);

    let _ = sqlx::query!("DELETE FROM users WHERE id = $1", created.id.as_str())
        .execute(db.pool_arc().expect("pool").as_ref())
        .await;
}

#[tokio::test]
async fn resolve_local_admin_email_missing_user_points_to_bootstrap_not_cloud_login() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let missing_username = format!("nonexistent_admin_{}", uuid::Uuid::new_v4());

    let err = resolve_local_admin_email(&missing_username, &db)
        .await
        .expect_err("missing admin row must error");

    let msg = format!("{:#}", err);
    assert!(
        msg.contains("systemprompt admin bootstrap"),
        "error must steer the user to the local bootstrap command, got: {msg}"
    );
    assert!(
        !msg.contains("cloud auth login"),
        "local-profile error must NOT mention cloud auth login, got: {msg}"
    );
    assert!(
        msg.contains(&missing_username),
        "error must name the missing username, got: {msg}"
    );
}
