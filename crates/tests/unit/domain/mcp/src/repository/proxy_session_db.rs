use systemprompt_identifiers::{McpServerId, SessionId, UserId};
use systemprompt_mcp::repository::{McpProxySessionRepository, McpSessionRepository};
use systemprompt_test_fixtures::{seed_user_row, test_db_pool};

fn session(prefix: &str) -> SessionId {
    SessionId::new(format!("{prefix}-{}", uuid::Uuid::new_v4().simple()))
}

async fn user(db: &systemprompt_database::DbPool) -> UserId {
    let id = UserId::new(uuid::Uuid::new_v4().to_string());
    seed_user_row(db, &id, &format!("{}@proxy-session.invalid", id.as_str()))
        .await
        .expect("seed user");
    id
}

fn server(prefix: &str) -> McpServerId {
    McpServerId::new(format!("{prefix}-{}", uuid::Uuid::new_v4().simple()))
}

#[tokio::test]
async fn a_bound_session_admits_only_its_server_and_principal() {
    let db = test_db_pool().await;
    let repo = McpProxySessionRepository::new(&db);
    let id = session("bind");
    let owner = user(&db).await;
    let srv = server("srv");
    repo.bind(&id, &srv, Some(&owner)).await.unwrap();

    assert!(repo.is_bound_to(&id, &srv, Some(&owner)).await.unwrap());
    assert!(
        !repo
            .is_bound_to(&id, &srv, Some(&user(&db).await))
            .await
            .unwrap()
    );
    assert!(!repo.is_bound_to(&id, &srv, None).await.unwrap());
    assert!(
        !repo
            .is_bound_to(&id, &server("other"), Some(&owner))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn an_unknown_session_is_bound_to_nobody() {
    let db = test_db_pool().await;
    let repo = McpProxySessionRepository::new(&db);
    assert!(
        !repo
            .is_bound_to(&session("unknown"), &server("srv"), Some(&user(&db).await))
            .await
            .unwrap()
    );
    assert!(
        !repo
            .is_bound_to(&session("unknown"), &server("srv"), None)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn binding_completes_the_row_the_backend_opened_and_keeps_its_initialize_params() {
    let db = test_db_pool().await;
    let repo = McpProxySessionRepository::new(&db);
    let backend = McpSessionRepository::new(&db);
    let id = session("backend-first");
    let srv = server("srv");
    let owner = user(&db).await;
    backend.create(&id, None, Some(&srv)).await.unwrap();
    backend
        .store_initialize_params(&id, &serde_json::json!({"protocolVersion": "2025-06-18"}))
        .await
        .unwrap();

    repo.bind(&id, &srv, Some(&owner)).await.unwrap();

    assert!(repo.is_bound_to(&id, &srv, Some(&owner)).await.unwrap());
    assert_eq!(
        backend.find_initialize_params(&id).await.unwrap(),
        Some(serde_json::json!({"protocolVersion": "2025-06-18"}))
    );
}

#[tokio::test]
async fn a_second_binding_cannot_take_over_a_bound_session() {
    let db = test_db_pool().await;
    let repo = McpProxySessionRepository::new(&db);
    let id = session("takeover");
    let srv = server("srv");
    let owner = user(&db).await;
    let intruder = user(&db).await;
    repo.bind(&id, &srv, Some(&owner)).await.unwrap();
    repo.bind(&id, &server("other"), Some(&intruder))
        .await
        .unwrap();

    assert!(repo.is_bound_to(&id, &srv, Some(&owner)).await.unwrap());
    assert!(!repo.is_bound_to(&id, &srv, Some(&intruder)).await.unwrap());
}

#[tokio::test]
async fn an_expired_session_is_bound_to_nobody() {
    let db = test_db_pool().await;
    let repo = McpProxySessionRepository::new(&db);
    let id = session("expired");
    let srv = server("srv");
    let owner = user(&db).await;
    repo.bind(&id, &srv, Some(&owner)).await.unwrap();
    sqlx::query(
        "UPDATE mcp_sessions SET expires_at = NOW() - INTERVAL '1 hour' WHERE session_id = $1",
    )
    .bind(id.as_str())
    .execute(db.write_pool().as_ref())
    .await
    .unwrap();

    assert!(!repo.is_bound_to(&id, &srv, Some(&owner)).await.unwrap());
}
