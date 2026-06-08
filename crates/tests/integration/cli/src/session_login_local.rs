//! Regression guards for `systemprompt admin session login` on local profiles.
//!
//! The original bug: `login` unconditionally called
//! `CredentialsBootstrap::require()` to resolve the operator's email, so a
//! local-only checkout (no `~/.systemprompt/cloud/credentials.json`) was locked
//! out of `session login` the moment its session expired. `fetch_admin_user`
//! now resolves the admin by the `system_admin.username` recorded in the
//! profile, the same key the runtime resolves at boot, so a local profile never
//! touches cloud credentials.

use systemprompt_cli::admin::session::login_helpers::fetch_admin_user;
use systemprompt_database::DbPool;
use systemprompt_users::UserService;

async fn get_db() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn fetch_admin_user_returns_bootstrapped_user_by_name() {
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

    let resolved = fetch_admin_user(&db, &username, false, None)
        .await
        .expect("local admin resolves by name without cloud credentials");

    assert_eq!(resolved.name, username);
    assert_eq!(resolved.email, email);

    let _ = sqlx::query!("DELETE FROM users WHERE id = $1", created.id.as_str())
        .execute(db.pool_arc().expect("pool").as_ref())
        .await;
}

#[tokio::test]
async fn fetch_admin_user_missing_local_user_points_to_bootstrap_not_cloud_login() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let missing_username = format!("nonexistent_admin_{}", uuid::Uuid::new_v4());

    let err = fetch_admin_user(&db, &missing_username, false, None)
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
