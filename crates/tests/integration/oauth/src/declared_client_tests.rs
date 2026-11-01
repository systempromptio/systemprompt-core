use crate::setup_test_db;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ClientId;
use systemprompt_oauth::repository::{ClientRepository, DeclaredClientRow, DeclaredClientWrite};
use uuid::Uuid;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

async fn upsert(db: &DbPool, write: DeclaredClientWrite<'_>) {
    let pool = db.write_pool();
    let mut tx = pool.begin().await.expect("begin");
    ClientRepository::upsert_declared_on(&mut tx, &write)
        .await
        .expect("upsert");
    tx.commit().await.expect("commit");
}

async fn declared(db: &DbPool, id: &ClientId) -> Option<DeclaredClientRow> {
    let pool = db.write_pool();
    let mut conn = pool.acquire().await.expect("conn");
    ClientRepository::list_declared_on(&mut conn)
        .await
        .expect("list")
        .into_iter()
        .find(|row| &row.client_id == id)
}

#[tokio::test]
async fn a_declared_client_is_written_rewritten_and_released() {
    let db = setup_test_db().await;
    let id = ClientId::new(format!("declared_{}", Uuid::new_v4().simple()));
    let owner = systemprompt_test_fixtures::fixture_user_id();
    let redirects = strings(&["https://b.example/cb", "https://a.example/cb"]);
    let grants = strings(&["refresh_token", "authorization_code"]);
    let scopes = strings(&["profile", "openid"]);
    upsert(
        &db,
        DeclaredClientWrite {
            client_id: &id,
            client_name: "Declared",
            owner_user_id: &owner,
            is_active: true,
            secret_key: Some("clients.declared"),
            secret_hash: Some("$2b$hash"),
            redirect_uris: &redirects,
            grant_types: &grants,
            scopes: &scopes,
            source: "identity/clients.yaml",
        },
    )
    .await;

    let row = declared(&db, &id).await.expect("declared row");
    assert_eq!(row.client_name, "Declared");
    assert_eq!(row.owner_user_id, owner);
    assert!(row.is_active);
    assert_eq!(row.secret_key.as_deref(), Some("clients.declared"));
    assert_eq!(
        row.grant_types,
        strings(&["authorization_code", "refresh_token"])
    );
    assert_eq!(row.scopes, strings(&["openid", "profile"]));
    assert_eq!(row.redirect_uris.len(), 2);

    let stored = ClientRepository::new(&db)
        .find_by_client_id(&id)
        .await
        .expect("find")
        .expect("client");
    assert_eq!(stored.token_endpoint_auth_method, "client_secret_basic");

    let narrowed = strings(&["openid"]);
    upsert(
        &db,
        DeclaredClientWrite {
            client_id: &id,
            client_name: "Renamed",
            owner_user_id: &owner,
            is_active: true,
            secret_key: None,
            secret_hash: None,
            redirect_uris: &redirects[..1],
            grant_types: &grants[1..],
            scopes: &narrowed,
            source: "identity/clients.yaml",
        },
    )
    .await;
    let row = declared(&db, &id).await.expect("still declared");
    assert_eq!(row.client_name, "Renamed");
    assert!(row.is_active);
    assert_eq!(row.secret_key, None);
    assert_eq!(row.redirect_uris, strings(&["https://b.example/cb"]));
    assert_eq!(row.grant_types, strings(&["authorization_code"]));
    assert_eq!(row.scopes, narrowed);
    let stored = ClientRepository::new(&db)
        .find_by_client_id(&id)
        .await
        .expect("find")
        .expect("client");
    assert_eq!(stored.token_endpoint_auth_method, "none");

    let pool = db.write_pool();
    let mut conn = pool.acquire().await.expect("conn");
    ClientRepository::release_declared_on(&mut conn, &id)
        .await
        .expect("release");
    drop(conn);
    assert!(
        declared(&db, &id).await.is_none(),
        "a released client is no longer declared"
    );
    let released = ClientRepository::new(&db)
        .find_by_client_id(&id)
        .await
        .expect("find");
    assert!(released.is_none_or(|c| !c.is_active));
}
