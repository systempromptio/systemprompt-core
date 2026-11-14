//! A Slack signing secret or Teams app password whose named ref points at a
//! built-in secret name (`oauth_at_rest_pepper`) resolves exactly as the
//! platform resolves it: the Slack handshake signed with that value is
//! accepted, and the Teams activity is acked rather than refused as an
//! unresolvable secret.

use axum::http::StatusCode;
use systemprompt_test_fixtures::{
    TEST_TEAMS_UNRESOLVABLE_APP_ID, TEST_TEAMS_UNRESOLVABLE_TENANT_ID, init_services_bootstrap,
    messaging_config_yaml_with_teams_endpoints, test_app_context_on, test_db_pool,
};
use tower::ServiceExt;
use wiremock::MockServer;

use super::common::body_to_string;
use super::{routes_slack, routes_teams};

const BUILT_IN: &str = "oauth_at_rest_pepper";

fn built_in_pepper(ctx: &systemprompt_runtime::AppContext) -> String {
    ctx.secrets()
        .get()
        .get(BUILT_IN)
        .cloned()
        .expect("the at-rest pepper is always present")
}

#[tokio::test]
async fn slack_signing_secret_ref_to_a_built_in_name_verifies() -> anyhow::Result<()> {
    let yaml = messaging_config_yaml_with_teams_endpoints(None).replace(
        "signing_secret_ref: slack_signing_secret",
        &format!("signing_secret_ref: {BUILT_IN}"),
    );
    let b = init_services_bootstrap(&yaml);
    let pool = test_db_pool().await;
    let ctx = test_app_context_on(&b, &pool);
    let secret = built_in_pepper(&ctx);

    let body = r#"{"type":"url_verification","challenge":"built-in-chal"}"#;
    let resp = routes_slack::router(&ctx)
        .oneshot(routes_slack::signed_post("/events", body, &secret))
        .await?;
    let (status, text) = body_to_string(resp).await?;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(text, "built-in-chal");
    Ok(())
}

#[tokio::test]
async fn teams_app_password_ref_to_a_built_in_name_resolves() -> anyhow::Result<()> {
    let connector = MockServer::start().await;
    let yaml = messaging_config_yaml_with_teams_endpoints(Some((
        &format!("{}/openid", connector.uri()),
        &format!("{}/token", connector.uri()),
    )))
    .replace(
        "app_password_ref: teams_app_password_unresolvable",
        &format!("app_password_ref: {BUILT_IN}"),
    );
    let b = init_services_bootstrap(&yaml);
    let pool = test_db_pool().await;
    let ctx = test_app_context_on(&b, &pool);

    let signing = systemprompt_test_fixtures::next_test_key();
    routes_teams::mount_connector_jwks(&connector, &signing).await;
    let token =
        routes_teams::mint_for_app(&signing, &connector.uri(), TEST_TEAMS_UNRESOLVABLE_APP_ID);
    let body = routes_teams::activity_json(TEST_TEAMS_UNRESOLVABLE_TENANT_ID, &connector.uri());
    let resp = routes_teams::router(&ctx)
        .oneshot(routes_teams::post_messages(&body, Some(&token)))
        .await?;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "a built-in secret name resolves; only a missing secret is a 503"
    );
    Ok(())
}
