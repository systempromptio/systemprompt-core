//! The bridge routes, driven through the gateway router, for a host that
//! configures itself.
//!
//! On an instance that declares `harness` the preference route stores a
//! user's choice without touching the hosts the bridge configures, and the
//! manifest route computes its minimum bridge version over all three host
//! fields it signs: the enabled hosts, the keys of the protocol filters and
//! the keys of the extension lists. On an instance that withholds the host
//! both preference routes refuse it, whatever the write.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::bridge::host::{ConfigOwner, HostKind};
use systemprompt_models::bridge::ids::HostExtensionId;
use systemprompt_models::bridge::manifest::{
    SignedManifest, SignedManifestEnvelope, min_bridge_version,
};
use systemprompt_test_fixtures::{
    AppContextSeeds as _, AuthedFixture, TestBootstrap, fixture_config, init_isolated_bootstrap,
    seed_bridge_credential, seed_user_row, test_db_pool,
};
use tower::ServiceExt;

const DECLARING: &str = r#"external_agents:
  harness:
    id: harness
    display_name: Harness
    kind: desktop_app
    enabled: true
    extensions:
      - drive
"#;

const WITHHOLDING: &str = r#"external_agents:
  harness:
    id: harness
    display_name: Harness
    kind: desktop_app
    enabled: false
"#;

// Each bootstrap owns the tempdir holding the services tree the handlers read
// on every request, so it has to outlive the test. A test uses one of the two.
static DECLARING_BOOT: OnceLock<TestBootstrap> = OnceLock::new();
static WITHHOLDING_BOOT: OnceLock<TestBootstrap> = OnceLock::new();

async fn router_over(
    boot: &'static OnceLock<TestBootstrap>,
    services: &'static str,
    email: &str,
) -> (Router, AuthedFixture) {
    let boot = boot.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", services));
    let pool = test_db_pool().await;
    let ctx = crate::support::context_on(
        boot,
        fixture_config(&boot.database_url),
        Arc::new(AllowAllFilter),
    )
    .await;
    let owner = ctx.system_admin().id();
    seed_user_row(&pool, owner, &format!("{owner}@harness-routes.invalid"))
        .await
        .expect("seed the organisation owner");
    let app = systemprompt_api::routes::gateway::gateway_router(&ctx, ctx.admitted(), ctx.kernel())
        .expect("gateway router");
    let user = seed_bridge_credential(&pool, email)
        .await
        .expect("bridge credential");
    (app, user)
}

async fn post(
    app: &Router,
    user: &AuthedFixture,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", user.jwt.as_str()),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("the route answers");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    (status, serde_json::from_slice(&bytes).expect("a JSON body"))
}

async fn manifest(app: &Router, user: &AuthedFixture) -> SignedManifest {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/bridge/manifest")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", user.jwt.as_str()),
                )
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("the manifest route answers");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let envelope: SignedManifestEnvelope =
        serde_json::from_slice(&bytes).expect("a signed envelope");
    assert!(!envelope.signature.as_str().is_empty(), "signed");
    serde_json::from_str(&envelope.payload).expect("the payload is the manifest")
}

fn bridge_configured() -> Vec<HostKind> {
    HostKind::ALL
        .into_iter()
        .filter(|host| host.config_owner() == ConfigOwner::Bridge)
        .collect()
}

fn harness_floor() -> semver::Version {
    HostKind::Harness
        .min_bridge_version()
        .expect("harness names the first bridge that parses it")
}

const ENABLED_HOSTS: &str = "/bridge/profile/enabled_hosts";
const MODEL_FILTER: &str = "/bridge/profile/host-model-filter";

#[tokio::test]
async fn enabling_harness_adds_it_to_the_signed_manifest_and_raises_its_floor() {
    let (app, user) = router_over(&DECLARING_BOOT, DECLARING, "harness-on@example.invalid").await;

    let before = manifest(&app, &user).await;
    assert_eq!(before.enabled_hosts, bridge_configured());
    assert!(before.host_extensions.is_empty());
    assert_eq!(
        before.min_bridge_version,
        Some(min_bridge_version()),
        "declaring the host asks nothing new of a user who has not turned it on"
    );

    let (status, body) = post(
        &app,
        &user,
        ENABLED_HOSTS,
        serde_json::json!({"host_id": "harness", "enabled": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let after = manifest(&app, &user).await;
    assert_eq!(
        after.enabled_hosts,
        HostKind::ALL.to_vec(),
        "the first preference a user stores being harness switches no bridge host off"
    );
    assert_eq!(
        after.host_extensions.get(&HostKind::Harness),
        Some(&vec![HostExtensionId::try_new("drive").expect("id")])
    );
    assert_eq!(after.min_bridge_version, Some(harness_floor()));

    let (status, body) = post(
        &app,
        &user,
        ENABLED_HOSTS,
        serde_json::json!({"host_id": "harness", "enabled": false}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "harness is never the last host: {body}"
    );
    let off = manifest(&app, &user).await;
    assert_eq!(off.enabled_hosts, bridge_configured());
    assert!(off.host_extensions.is_empty());
    assert_eq!(off.min_bridge_version, Some(min_bridge_version()));
}

#[tokio::test]
async fn a_filter_key_alone_raises_the_floor_of_the_signed_manifest() {
    let (app, user) =
        router_over(&DECLARING_BOOT, DECLARING, "harness-filter@example.invalid").await;

    let (status, body) = post(
        &app,
        &user,
        MODEL_FILTER,
        serde_json::json!({"host_id": "harness", "model_protocols": ["claude"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let signed = manifest(&app, &user).await;
    assert_eq!(
        signed.enabled_hosts,
        bridge_configured(),
        "a filter does not enable the host"
    );
    assert!(signed.host_model_protocols.contains_key(&HostKind::Harness));
    assert_eq!(
        signed.min_bridge_version,
        Some(harness_floor()),
        "the route passes the filter keys to the floor: a bridge that cannot parse the key is \
         told to upgrade"
    );
}

#[tokio::test]
async fn a_withheld_harness_is_refused_by_both_preference_routes() {
    let (app, user) = router_over(
        &WITHHOLDING_BOOT,
        WITHHOLDING,
        "harness-withheld@example.invalid",
    )
    .await;

    let writes = [
        (
            ENABLED_HOSTS,
            serde_json::json!({"host_id": "harness", "enabled": true}),
        ),
        (
            ENABLED_HOSTS,
            serde_json::json!({"host_id": "harness", "enabled": false}),
        ),
        (
            MODEL_FILTER,
            serde_json::json!({"host_id": "harness", "model_protocols": ["claude"]}),
        ),
        (
            MODEL_FILTER,
            serde_json::json!({"host_id": "harness", "model_protocols": null}),
        ),
    ];
    for (uri, body) in writes {
        let (status, answer) = post(&app, &user, uri, body.clone()).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{uri} {body}: {answer}"
        );
        assert_eq!(
            answer["error_key"].as_str(),
            Some("host_disabled"),
            "{uri} {body}: {answer}"
        );
    }

    let signed = manifest(&app, &user).await;
    assert_eq!(signed.enabled_hosts, bridge_configured());
    assert!(signed.host_model_protocols.is_empty());
    assert_eq!(signed.min_bridge_version, Some(min_bridge_version()));

    let (status, body) = post(
        &app,
        &user,
        ENABLED_HOSTS,
        serde_json::json!({"host_id": "claude-code", "enabled": false}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a host the instance serves is still written: {body}"
    );
}
