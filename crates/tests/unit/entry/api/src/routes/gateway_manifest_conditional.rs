//! A manifest fetch that sends back the entity tag it was given answers 304
//! with an empty body while the catalogue is unchanged, so a bridge
//! revalidates its stored signed envelope instead of downloading it again.

use std::sync::{Arc, OnceLock};

use axum::body::to_bytes;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use systemprompt_api::routes::gateway::bridge_manifest;
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_marketplace::{AllowAllFilter, MarketplaceFilter};
use systemprompt_models::bridge::manifest::{MANIFEST_SCHEMA_HEADER, SignedManifestEnvelope};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context_with, init_isolated_bootstrap, install_test_signing_key,
    seed_bridge_credential, seed_user_row, test_db_pool,
};
use systemprompt_traits::AppContext as _;

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", "plugins: {}\n"))
}

fn boot_paths(boot: &TestBootstrap) -> PathsConfig {
    PathsConfig {
        system: boot.system_path.display().to_string(),
        services: boot.services_path.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: Some(boot.system_path.join("web").display().to_string()),
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
        shipped_files: None,
        data: None,
    }
}

struct Harness {
    ctx: AppContext,
    extractor: Arc<JwtContextExtractor>,
    jwt: String,
}

async fn harness(mailbox: &str) -> Harness {
    let boot = boot();
    install_test_signing_key();
    let pool = test_db_pool().await;
    let ctx = fixture_app_context_with(
        &pool,
        &boot.database_url,
        boot_paths(boot),
        Arc::new(AllowAllFilter) as Arc<dyn MarketplaceFilter>,
    )
    .expect("fixture context");
    let owner = ctx.system_admin().id();
    seed_user_row(
        &pool,
        owner,
        &format!("{owner}@manifest-conditional.invalid"),
    )
    .await
    .expect("seed the organisation owner");
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.session_provider().expect("session provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(ctx.oauth_repositories().oauth.clone()),
        ctx.config().jwt_issuer.clone(),
    ));
    let consumer = seed_bridge_credential(&pool, mailbox)
        .await
        .expect("consumer credential");
    Harness {
        ctx: (*ctx).clone(),
        extractor,
        jwt: consumer.jwt.as_str().to_owned(),
    }
}

async fn fetch(harness: &Harness, extra: &[(&'static str, &str)]) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", harness.jwt)
            .parse()
            .expect("bearer header"),
    );
    for (name, value) in extra {
        headers.insert(*name, value.parse().expect("header value"));
    }
    bridge_manifest::manifest(Arc::clone(&harness.extractor), harness.ctx.clone(), headers)
        .await
        .expect("a signed-in consumer receives a manifest")
}

fn entity_tag(response: &Response) -> String {
    response
        .headers()
        .get(header::ETAG)
        .expect("a manifest carries an entity tag")
        .to_str()
        .expect("ascii tag")
        .to_owned()
}

#[tokio::test]
async fn a_matching_entity_tag_answers_not_modified_with_no_body() {
    let harness = harness("manifest-conditional@example.invalid").await;

    let first = fetch(&harness, &[]).await;
    assert_eq!(first.status(), StatusCode::OK);
    let tag = entity_tag(&first);
    let body = to_bytes(first.into_body(), usize::MAX).await.expect("body");
    let envelope: SignedManifestEnvelope = serde_json::from_slice(&body).expect("envelope");
    assert!(!envelope.payload.is_empty());

    let revalidated = fetch(&harness, &[("if-none-match", tag.as_str())]).await;
    assert_eq!(revalidated.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(entity_tag(&revalidated), tag);
    let empty = to_bytes(revalidated.into_body(), usize::MAX)
        .await
        .expect("body");
    assert!(empty.is_empty(), "a 304 carries no envelope");

    let rebuilt = fetch(
        &harness,
        &[
            ("if-none-match", tag.as_str()),
            ("cache-control", "no-cache"),
        ],
    )
    .await;
    assert_eq!(
        rebuilt.status(),
        StatusCode::NOT_MODIFIED,
        "no-cache revalidates against a rebuild rather than forcing a resend"
    );

    let stale = fetch(&harness, &[("if-none-match", "\"stale\"")]).await;
    assert_eq!(stale.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_schema_two_response_is_signed_and_tagged() {
    let harness = harness("manifest-schema-two@example.invalid").await;
    let response = fetch(&harness, &[(MANIFEST_SCHEMA_HEADER, "2")]).await;
    assert_eq!(response.status(), StatusCode::OK);
    entity_tag(&response);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let envelope: SignedManifestEnvelope = serde_json::from_slice(&body).expect("envelope");
    let payload: serde_json::Value = serde_json::from_str(&envelope.payload).expect("payload");
    assert_eq!(
        payload["min_schema_version"], 1,
        "0.67 bridges are never refused"
    );
}
