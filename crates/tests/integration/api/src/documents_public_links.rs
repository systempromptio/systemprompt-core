//! A document's public link: created by a sharer, opened anonymously at
//! `/s/{token}` with no session and no cookie, the token stored only as its
//! hash, one undifferentiated 404 for every refusal, a password lockout, and
//! the external-sharing policy re-checked on every open.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use systemprompt_api::routes::documents::files_host_handles;
use systemprompt_files::{DocumentsConfig, ExternalSharingConfig};
use systemprompt_files_entry::http::documents::{DocumentsState, public_link};
use systemprompt_models::documents::DocumentFormat;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    AuthedFixture, fixture_admitted, fixture_app_context_on_bootstrap, fixture_config,
    init_documents_bootstrap, seed_bridge_credential, test_db_pool,
};

use super::documents::{Reply, app, create_markdown, id_of, json_request, request, send};

const PASSWORD: &str = "correct horse battery";

pub(super) struct Instance {
    pub(super) ctx: Arc<AppContext>,
    pub(super) state: DocumentsState,
    pub(super) public: Router,
}

pub(super) async fn instance(sharing: ExternalSharingConfig) -> Result<Instance> {
    let bootstrap = init_documents_bootstrap(DocumentsConfig {
        enabled: true,
        external_sharing: sharing,
        ..DocumentsConfig::default()
    });
    let ctx = fixture_app_context_on_bootstrap(
        &bootstrap,
        fixture_config(&bootstrap.database_url),
        fixture_admitted(),
    )
    .await?;
    let public = public_link::router(
        systemprompt_test_fixtures::documents_handles(&ctx)
            .service
            .clone()
            .expect("the document store is enabled"),
        files_host_handles(&ctx).client_ip,
    )
    .layer(MockConnectInfo(SocketAddr::from(([203, 0, 113, 7], 4000))));
    Ok(Instance {
        state: crate::common::documents_state(&ctx)?,
        ctx,
        public,
    })
}

fn links_on() -> ExternalSharingConfig {
    ExternalSharingConfig {
        public_links: true,
        ..ExternalSharingConfig::default()
    }
}

async fn create_link(at: &Instance, user: &AuthedFixture, id: &str, body: &Value) -> Result<Reply> {
    send(
        app(&at.state, user),
        json_request(Method::POST, &format!("/{id}/public-links"), body),
    )
    .await
}

async fn token_of(at: &Instance, user: &AuthedFixture, id: &str, body: &Value) -> Result<String> {
    let created = create_link(at, user, id, body).await?;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
    Ok(created.json()["token"].as_str().expect("token").to_owned())
}

pub(super) async fn open(at: &Instance, token: &str) -> Result<Reply> {
    let req = request(Method::GET, &format!("/{token}"))
        .body(Body::empty())
        .expect("request");
    send(at.public.clone(), req).await
}

async fn open_with(at: &Instance, token: &str, password: &str) -> Result<Reply> {
    let req = request(Method::POST, &format!("/{token}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!(
            "password={}",
            password.replace(' ', "+")
        )))
        .expect("request");
    send(at.public.clone(), req).await
}

pub(super) fn hash_of(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(super) async fn markdown(at: &Instance, owner: &AuthedFixture) -> Result<String> {
    Ok(id_of(&create_markdown(&at.state, owner).await?))
}

#[tokio::test]
async fn a_public_link_opens_anonymously_with_no_cookie_and_stores_only_its_hash() -> Result<()> {
    let pool = test_db_pool().await;
    let at = instance(links_on()).await?;
    let owner = seed_bridge_credential(&pool, "public-link-open").await?;
    let id = markdown(&at, &owner).await?;

    let created = create_link(&at, &owner, &id, &json!({})).await?;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
    let token = created.json()["token"].as_str().expect("token").to_owned();
    assert_eq!(created.json()["path"], format!("/s/{token}"));
    assert_eq!(created.json()["link"]["has_password"], false);

    let stored: Vec<String> = sqlx::query_scalar(
        "SELECT token_hash FROM document_public_links WHERE document_id = $1::uuid",
    )
    .bind(&id)
    .fetch_all(&*pool.pool())
    .await?;
    assert_eq!(stored, vec![hash_of(&token)]);

    let reply = open(&at, &token).await?;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(!reply.body.is_empty());
    assert!(reply.headers.get(header::SET_COOKIE).is_none());
    assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(reply.headers[header::REFERRER_POLICY], "no-referrer");
    assert_eq!(
        reply.headers[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let csp = reply.headers[header::CONTENT_SECURITY_POLICY].to_str()?;
    assert!(csp.starts_with("sandbox"), "{csp}");

    let listed = send(
        app(&at.state, &owner),
        request(Method::GET, &format!("/{id}/public-links"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(listed.json()["links"][0]["open_count"], 1);
    assert!(!String::from_utf8_lossy(&listed.body).contains(&token));
    Ok(())
}

#[tokio::test]
async fn revoked_expired_and_unknown_links_answer_one_404() -> Result<()> {
    let pool = test_db_pool().await;
    let at = instance(links_on()).await?;
    let owner = seed_bridge_credential(&pool, "public-link-gone").await?;
    let id = markdown(&at, &owner).await?;

    let unknown = open(&at, &"A".repeat(43)).await?;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    let malformed = open(&at, "short").await?;
    assert_eq!(malformed.body, unknown.body);

    let created = create_link(&at, &owner, &id, &json!({})).await?;
    let token = created.json()["token"].as_str().expect("token").to_owned();
    let link_id = created.json()["link"]["id"]
        .as_str()
        .expect("id")
        .to_owned();
    let revoked = send(
        app(&at.state, &owner),
        request(Method::DELETE, &format!("/{id}/public-links/{link_id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT);
    let after = open(&at, &token).await?;
    assert_eq!((after.status, &after.body), (unknown.status, &unknown.body));

    let expiring = token_of(&at, &owner, &id, &json!({})).await?;
    sqlx::query(
        "UPDATE document_public_links SET expires_at = now() - interval '1 minute'
         WHERE token_hash = $1",
    )
    .bind(hash_of(&expiring))
    .execute(&*pool.pool())
    .await?;
    let expired = open(&at, &expiring).await?;
    assert_eq!(
        (expired.status, &expired.body),
        (unknown.status, &unknown.body)
    );
    Ok(())
}

#[tokio::test]
async fn a_password_link_locks_out_after_five_wrong_attempts() -> Result<()> {
    let pool = test_db_pool().await;
    let at = instance(links_on()).await?;
    let owner = seed_bridge_credential(&pool, "public-link-password").await?;
    let id = markdown(&at, &owner).await?;

    let short = create_link(&at, &owner, &id, &json!({ "password": "short" })).await?;
    assert_eq!(short.status, StatusCode::BAD_REQUEST, "{}", short.json());

    let token = token_of(&at, &owner, &id, &json!({ "password": PASSWORD })).await?;
    let bare = open(&at, &token).await?;
    assert_eq!(bare.status, StatusCode::NOT_FOUND);
    assert_eq!(
        open_with(&at, &token, PASSWORD).await?.status,
        StatusCode::OK
    );

    for _ in 0..5 {
        let wrong = open_with(&at, &token, "not the password").await?;
        assert_eq!((wrong.status, &wrong.body), (bare.status, &bare.body));
    }
    let locked = open_with(&at, &token, PASSWORD).await?;
    assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS);
    Ok(())
}

#[tokio::test]
async fn the_policy_gates_creation_and_is_rechecked_on_every_open() -> Result<()> {
    let pool = test_db_pool().await;
    let off = instance(ExternalSharingConfig::default()).await?;
    let open_policy = instance(links_on()).await?;
    let deny_markdown = instance(ExternalSharingConfig {
        deny_formats: vec![DocumentFormat::Markdown],
        ..links_on()
    })
    .await?;
    let owner = seed_bridge_credential(&pool, "public-link-policy").await?;
    let id = markdown(&open_policy, &owner).await?;

    let refused = create_link(&off, &owner, &id, &json!({})).await?;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.json());
    let denied = create_link(&deny_markdown, &owner, &id, &json!({})).await?;
    assert_eq!(denied.status, StatusCode::FORBIDDEN, "{}", denied.json());

    let token = token_of(&open_policy, &owner, &id, &json!({})).await?;
    assert_eq!(open(&open_policy, &token).await?.status, StatusCode::OK);
    assert_eq!(
        open(&deny_markdown, &token).await?.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(open(&off, &token).await?.status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn only_a_sharer_creates_a_public_link() -> Result<()> {
    let pool = test_db_pool().await;
    let at = instance(links_on()).await?;
    let owner = seed_bridge_credential(&pool, "public-link-owner").await?;
    let editor = seed_bridge_credential(&pool, "public-link-editor").await?;
    let id = markdown(&at, &owner).await?;
    let granted = send(
        app(&at.state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/grants"),
            &json!({
                "subject_kind": "user",
                "subject_value": editor.user_id.as_str(),
                "level": "edit"
            }),
        ),
    )
    .await?;
    assert_eq!(granted.status, StatusCode::CREATED, "{}", granted.json());

    let refused = create_link(&at, &editor, &id, &json!({})).await?;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.json());
    Ok(())
}

#[tokio::test]
async fn the_mounted_public_link_route_opens_without_a_session_under_the_public_policy()
-> Result<()> {
    use axum::extract::ConnectInfo;
    use systemprompt_api::services::server::routes::Composed;
    use systemprompt_api::services::server::{ReadinessState, setup_api_server};

    let pool = test_db_pool().await;
    let at = instance(links_on()).await?;
    let owner = seed_bridge_credential(&pool, "public-link-mounted").await?;
    let id = markdown(&at, &owner).await?;
    let token = token_of(&at, &owner, &id, &json!({})).await?;
    let server = setup_api_server(
        &at.ctx,
        Composed {
            admitted: at.ctx.admitted(),
            kernel: at.ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    let call = |method: Method, uri: String, body: Option<&'static str>| {
        let mut builder = request(method, &uri)
            .header(header::HOST, "127.0.0.1")
            .header(header::USER_AGENT, "documents-test/1.0");
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        }
        let mut req = builder
            .body(body.map_or_else(Body::empty, Body::from))
            .expect("request");
        req.extensions_mut().insert(ConnectInfo(
            "203.0.113.9:40000".parse::<SocketAddr>().expect("addr"),
        ));
        send(server.clone(), req)
    };

    let opened = call(Method::GET, format!("/s/{token}"), None).await?;
    assert_eq!(opened.status, StatusCode::OK);
    assert!(opened.headers.get(header::SET_COOKIE).is_none());
    assert_eq!(opened.headers[header::CACHE_CONTROL], "no-store");

    let unknown = call(Method::GET, format!("/s/{}", "A".repeat(43)), None).await?;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);

    let posted = call(
        Method::POST,
        format!("/s/{}", "B".repeat(43)),
        Some("password=wrong"),
    )
    .await?;
    assert_eq!(posted.status, StatusCode::NOT_FOUND);
    Ok(())
}
