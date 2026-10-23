//! Behaviour tests for `CredentialsBootstrap` on the Fly-container path:
//! strict validation failure, then the unvalidated override. The steps share
//! process environment variables, so they are sequenced inside one test.

use chrono::Duration;
use systemprompt_cloud::CredentialsBootstrap;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fly_bootstrap_lifecycle() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .and(header("Authorization", "Bearer env-token"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    unsafe {
        std::env::set_var("FLY_APP_NAME", "cov-cloud-app");
        std::env::set_var("SYSTEMPROMPT_API_TOKEN", "env-token");
        std::env::set_var("SYSTEMPROMPT_USER_EMAIL", "ops@example.com");
        std::env::set_var("SYSTEMPROMPT_API_URL", server.uri());
        std::env::remove_var("SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS");
    }

    let err = CredentialsBootstrap::load().await.unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("rejected by api.systemprompt.io"),
        "got {message}"
    );
    assert!(CredentialsBootstrap::require().await.is_err());

    unsafe { std::env::set_var("SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS", "1") };
    let creds = CredentialsBootstrap::load()
        .await
        .expect("unvalidated init succeeds")
        .expect("credentials loaded from environment");
    assert_eq!(creds.user_email.as_str(), "ops@example.com");
    assert_eq!(creds.api_url, server.uri());
    assert_eq!(creds.api_token.as_str(), "env-token");
    assert!(creds.last_validated_at.is_none());

    assert!(creds.expires_within(Duration::hours(1)));

    let required = CredentialsBootstrap::require()
        .await
        .expect("require after override");
    assert_eq!(required.api_token.as_str(), "env-token");

    let unvalidated = CredentialsBootstrap::read_unvalidated()
        .expect("read unvalidated")
        .expect("environment credentials");
    assert_eq!(unvalidated.user_email.as_str(), "ops@example.com");
}
