use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::prelude::{ChangeActor, ConfigPlane};
use systemprompt_manifest::secrets::Secrets;
use systemprompt_oauth_issuance::IdentityPlane;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::ConfigPlaneSet;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{ApplyOptions, ConfigApplyService, ConfigReconcileService};
use systemprompt_test_fixtures::DisposableDb;

const USERS: &str = "users:\n  - email: alice@example.com\n    name: Alice\n    roles: [admin]\n  \
                     - email: robot@example.com\n    name: Robot\n    kind: service\n";

const CLIENT: &str = "oauth_clients:\n  - client_id: portal\n    name: Portal\n    owner: \
                      alice@example.com\n    redirect_uris: [https://portal.example.com/cb]\n    \
                      scopes: [read]\n    secret_key: portal_secret\n";

fn write(root: &std::path::Path, file: &str, body: &str) {
    let path = root.join("identity").join(file);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, body).expect("write");
}

fn secrets() -> Arc<Secrets> {
    Arc::new(
        Secrets::parse(
            r#"{"oauth_at_rest_pepper":"0123456789abcdef0123456789abcdef","database_url":"postgres://x","portal_secret":"s3cr3t-value"}"#,
        )
        .expect("secrets"),
    )
}

fn service(
    pool: &systemprompt_database::DbPool,
    root: &std::path::Path,
    deps: Dependencies,
) -> ConfigApplyService {
    let planes: Vec<Arc<dyn ConfigPlane>> = vec![Arc::new(IdentityPlane)];
    let set = ConfigPlaneSet::new(planes, Default::default(), root.to_path_buf()).expect("set");
    ConfigApplyService::new(ConfigReconcileService::new(
        set,
        ConfigReconcileRepository::new(pool),
        deps,
    ))
}

fn options() -> ApplyOptions<'static> {
    ApplyOptions {
        planes: &[],
        actor: ChangeActor::Cli,
        reason: "identity",
        dry_run: false,
        wait: Duration::from_secs(30),
    }
}

#[tokio::test]
async fn declared_users_and_clients_are_written_then_released() {
    let db = DisposableDb::with_schema("identityplane").await;
    let pool = db.test_pool().await;
    let root = tempfile::tempdir().expect("tempdir");
    write(root.path(), "identity.yaml", &format!("{USERS}{CLIENT}"));
    let svc = service(&pool, root.path(), Dependencies::new().with(secrets()));

    let first = svc.apply(&options()).await.expect("apply");
    assert_eq!(first.applied(), 3, "{first:?}");
    let robot: (String, Option<String>) =
        sqlx::query_as("SELECT account_kind, declared_source FROM users WHERE email = $1")
            .bind("robot@example.com")
            .fetch_one(&*pool.pool())
            .await
            .expect("robot");
    assert_eq!(robot.0, "service");
    assert_eq!(robot.1.as_deref(), Some("code:identity/identity.yaml"));
    let hash: Option<String> =
        sqlx::query_scalar("SELECT client_secret_hash FROM oauth_clients WHERE client_id = $1")
            .bind("portal")
            .fetch_one(&*pool.pool())
            .await
            .expect("client");
    let hash = hash.expect("hashed secret");
    assert!(!hash.contains("s3cr3t-value"));

    let second = svc.apply(&options()).await.expect("reapply");
    assert_eq!(second.applied(), 0, "{second:?}");

    write(
        root.path(),
        "identity.yaml",
        "users:\n  - email: alice@example.com\n    name: Alice\n    roles: [admin]\n",
    );
    let third = svc.apply(&options()).await.expect("release");
    assert_eq!(third.applied(), 2, "{third:?}");
    let released: (String, Option<String>) =
        sqlx::query_as("SELECT status, declared_source FROM users WHERE email = $1")
            .bind("robot@example.com")
            .fetch_one(&*pool.pool())
            .await
            .expect("released");
    assert_eq!(released.0, "inactive");
    assert!(released.1.is_none());
    let client: (bool, Option<String>) =
        sqlx::query_as("SELECT is_active, declared_source FROM oauth_clients WHERE client_id = $1")
            .bind("portal")
            .fetch_one(&*pool.pool())
            .await
            .expect("client row");
    assert!(!client.0);
    assert!(client.1.is_none());
    db.drop_now().await;
}

#[tokio::test]
async fn a_client_whose_owner_is_undeclared_is_refused() {
    let db = DisposableDb::with_schema("identityowner").await;
    let pool = db.test_pool().await;
    let root = tempfile::tempdir().expect("tempdir");
    write(root.path(), "clients.yaml", CLIENT);
    let svc = service(&pool, root.path(), Dependencies::new().with(secrets()));

    let err = svc.apply(&options()).await.expect_err("owner missing");
    assert!(format!("{err:?}").contains("is not a user"), "{err:?}");
    db.drop_now().await;
}

#[tokio::test]
async fn a_secret_key_absent_from_the_secrets_document_is_refused() {
    let db = DisposableDb::with_schema("identitysecret").await;
    let pool = db.test_pool().await;
    let root = tempfile::tempdir().expect("tempdir");
    write(root.path(), "identity.yaml", &format!("{USERS}{CLIENT}"));
    let svc = service(&pool, root.path(), Dependencies::new());

    let err = svc.apply(&options()).await.expect_err("secret missing");
    assert!(format!("{err:?}").contains("portal_secret"), "{err:?}");
    let users: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE declared_source IS NOT NULL")
            .fetch_one(&*pool.pool())
            .await
            .expect("count");
    assert_eq!(users, 0);
    db.drop_now().await;
}

#[tokio::test]
async fn export_renders_the_declared_rows_back_to_yaml() {
    let db = DisposableDb::with_schema("identityexport").await;
    let pool = db.test_pool().await;
    let root = tempfile::tempdir().expect("tempdir");
    write(root.path(), "identity.yaml", &format!("{USERS}{CLIENT}"));
    let svc = service(&pool, root.path(), Dependencies::new().with(secrets()));
    svc.apply(&options()).await.expect("apply");

    let exported = svc
        .reconcile()
        .export("identity")
        .await
        .expect("export")
        .expect("rows exist");
    assert!(exported.contains("alice@example.com"), "{exported}");
    assert!(exported.contains("client_id: portal"), "{exported}");
    assert!(exported.contains("secret_key: portal_secret"), "{exported}");
    assert!(!exported.contains("s3cr3t-value"));
    db.drop_now().await;
}

#[tokio::test]
async fn export_of_an_empty_plane_is_none() {
    let db = DisposableDb::with_schema("identityempty").await;
    let pool = db.test_pool().await;
    let root = tempfile::tempdir().expect("tempdir");
    let svc = service(&pool, root.path(), Dependencies::new());
    let exported = svc.reconcile().export("identity").await.expect("export");
    assert!(exported.is_none());
    db.drop_now().await;
}

#[tokio::test]
async fn a_client_declared_in_a_file_before_its_owner_is_written_after_the_owner() {
    let db = DisposableDb::with_schema("identityorder").await;
    let pool = db.test_pool().await;
    let root = tempfile::tempdir().expect("tempdir");
    write(root.path(), "clients.yaml", CLIENT);
    write(root.path(), "users.yaml", USERS);
    let svc = service(&pool, root.path(), Dependencies::new().with(secrets()));

    let first = svc.apply(&options()).await.expect("apply");
    assert_eq!(first.applied(), 3, "{first:?}");
    let source: Option<String> =
        sqlx::query_scalar("SELECT declared_source FROM oauth_clients WHERE client_id = $1")
            .bind("portal")
            .fetch_one(&*pool.pool())
            .await
            .expect("portal");
    assert_eq!(source.as_deref(), Some("code:identity/clients.yaml"));
}
