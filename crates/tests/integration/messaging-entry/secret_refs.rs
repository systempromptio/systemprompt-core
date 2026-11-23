//! Secret access for the Slack and Teams routes. A custom-secret ref resolves
//! and the request is accepted; the secret store an extension is handed
//! resolves no built-in platform secret; and the shared extension state holds
//! no unscoped secret lookup.

use axum::http::StatusCode;
use systemprompt_test_fixtures::{
    TEST_SLACK_SIGNING_SECRET, ensure_messaging_bootstrap, test_app_context,
};
use systemprompt_traits::SecretLookup;
use tower::ServiceExt;

use super::common::body_to_string;
use super::routes_slack;

#[tokio::test]
async fn a_custom_secret_ref_resolves_and_the_request_is_accepted() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let ctx = test_app_context(&b.database_url).await;

    let body = r#"{"type":"url_verification","challenge":"custom-chal"}"#;
    let resp = routes_slack::router(&ctx)
        .oneshot(routes_slack::signed_post(
            "/events",
            body,
            TEST_SLACK_SIGNING_SECRET,
        ))
        .await?;
    let (status, text) = body_to_string(resp).await?;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(text, "custom-chal");
    Ok(())
}

#[tokio::test]
async fn the_extension_secret_store_resolves_no_built_in_secret() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let ctx = test_app_context(&b.database_url).await;

    assert!(
        ctx.secrets().get().get("oauth_at_rest_pepper").is_some(),
        "the platform holds the pepper, so a None below is the store's scope"
    );
    let store = std::sync::Arc::clone(
        ctx.kernel()
            .state()
            .get::<std::sync::Arc<dyn systemprompt_traits::SecretStore>>()
            .expect("the secret store is a kernel seed"),
    );
    for name in ["signing_key_pem", "oauth_at_rest_pepper", "database_url"] {
        assert!(
            store.secret(name).is_none(),
            "an extension must not read the built-in secret {name}"
        );
    }
    assert!(
        ctx.kernel()
            .state()
            .is_sealed::<std::sync::Arc<dyn SecretLookup>>(),
        "the unscoped secret lookup is a sealed kernel seed"
    );
    assert!(
        ctx.kernel()
            .runtime_state()
            .find::<std::sync::Arc<dyn SecretLookup>>()
            .is_none(),
        "no member runtime context carries the unscoped secret lookup"
    );
    Ok(())
}
