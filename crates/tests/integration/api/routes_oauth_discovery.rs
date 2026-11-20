//! `oauth/discovery` handlers — the `/.well-known/openid-configuration`
//! variants. The handlers read the process-wide `Config`; we install a test
//! Config once at module load.

use axum::Router;
use axum::routing::get;
use systemprompt_api::routes::oauth;
use systemprompt_manifest::profile::UiSessionConfig;
use tower::ServiceExt;

use super::common::{body_to_string, empty_get, setup_ctx, setup_ctx_customised};

async fn discovery_app() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    Ok(discovery_router_over(&ctx))
}

fn discovery_router_over(ctx: &systemprompt_runtime::AppContext) -> Router {
    Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(oauth::discovery::handle_well_known),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth::discovery::handle_oauth_protected_resource),
        )
        .route(
            "/.well-known/oauth-protected-resource/{*path}",
            get(oauth::discovery::handle_oauth_protected_resource_with_path),
        )
        .with_state((*ctx).clone())
        .layer(axum::Extension(ctx.config_arc().clone()))
}

#[tokio::test]
async fn well_known_returns_openid_config() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn well_known_advertises_iss_parameter_support() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status}");
    let json: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(
        json["authorization_response_iss_parameter_supported"],
        serde_json::Value::Bool(true)
    );
    Ok(())
}

#[tokio::test]
async fn protected_resource_returns_metadata() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/oauth-protected-resource"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn well_known_advertises_token_exchange_and_id_jag() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status}");
    let json: serde_json::Value = serde_json::from_str(&body)?;

    let grants = json["grant_types_supported"]
        .as_array()
        .expect("grant_types_supported is an array");
    assert!(
        grants
            .iter()
            .any(|g| g == "urn:ietf:params:oauth:grant-type:token-exchange"),
        "token-exchange grant must be advertised: {json}"
    );
    for field in [
        "subject_token_types_supported",
        "issued_token_types_supported",
    ] {
        let types = json[field].as_array().expect("token-type array");
        assert!(
            types
                .iter()
                .any(|t| t == "urn:ietf:params:oauth:token-type:id-jag"),
            "{field} must advertise id-jag: {json}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn protected_resource_with_path_falls_back_to_default() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get(
            "/.well-known/oauth-protected-resource/unrelated/path",
        ))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn protected_resource_with_mcp_path_returns_metadata() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get(
            "/.well-known/oauth-protected-resource/mcp/my-server/mcp",
        ))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn well_known_advertises_the_id_jag_grant_profile() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status}");
    let json: serde_json::Value = serde_json::from_str(&body)?;

    let profiles = json["authorization_grant_profiles_supported"]
        .as_array()
        .expect("authorization_grant_profiles_supported is an array");
    assert!(
        profiles
            .iter()
            .any(|p| p == "urn:ietf:params:oauth:grant-profile:id-jag"),
        "EMA clients detect support through the grant profile: {body}"
    );

    let grants = json["grant_types_supported"]
        .as_array()
        .expect("grant_types_supported is an array");
    assert!(
        grants
            .iter()
            .any(|g| g == "urn:ietf:params:oauth:grant-type:jwt-bearer"),
        "the ID-JAG redemption grant must be advertised: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn well_known_advertises_registration_while_dcr_is_open() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status}");
    let json: serde_json::Value = serde_json::from_str(&body)?;
    assert!(
        json["registration_endpoint"]
            .as_str()
            .is_some_and(|e| e.ends_with("/api/v1/core/oauth/register")),
        "{json}"
    );
    Ok(())
}

#[tokio::test]
async fn well_known_advertises_the_ui_session_capability() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status}");
    let json: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(json["ui_session_supported"], true, "{body}");
    assert_eq!(
        json["ui_session"],
        serde_json::json!({
            "access_token_lifetime": 300,
            "session_lifetime": 43_200,
            "step_up_max_age": 600,
        }),
        "{body}"
    );
    assert!(!body.contains("client_id"), "{body}");
    Ok(())
}

#[tokio::test]
async fn well_known_advertises_the_configured_ui_session_lifetimes() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx_customised(|config| {
        config.ui_session = UiSessionConfig {
            access_ttl_secs: 120,
            absolute_secs: 3600,
            step_up_max_age_secs: 180,
        };
    })
    .await?;
    let resp = discovery_router_over(&ctx)
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status}");
    let json: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(
        json["ui_session"],
        serde_json::json!({
            "access_token_lifetime": 120,
            "session_lifetime": 3600,
            "step_up_max_age": 180,
        }),
        "{body}"
    );
    Ok(())
}
