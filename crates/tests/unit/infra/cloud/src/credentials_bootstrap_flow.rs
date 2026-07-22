//! Behaviour tests for the process-wide `CredentialsBootstrap` on the
//! Fly-container path. The bootstrap cell is a process-global `OnceLock`,
//! so the whole lifecycle (uninitialised -> strict validation failure ->
//! unvalidated override -> initialised accessors -> double-init rejection)
//! is sequenced inside one test.

use chrono::Duration;
use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError};
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

    assert!(matches!(
        CredentialsBootstrap::get(),
        Err(CredentialsBootstrapError::NotInitialized)
    ));
    assert!(!CredentialsBootstrap::is_initialized());
    assert!(!CredentialsBootstrap::expires_within(Duration::hours(1)));

    let err = CredentialsBootstrap::init().await.unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("rejected by api.systemprompt.io"),
        "got {message}"
    );
    assert!(!CredentialsBootstrap::is_initialized());

    unsafe { std::env::set_var("SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS", "1") };
    let creds = CredentialsBootstrap::init()
        .await
        .expect("unvalidated init succeeds")
        .expect("credentials loaded from environment");
    assert_eq!(creds.user_email.as_str(), "ops@example.com");
    assert_eq!(creds.api_url, server.uri());
    assert_eq!(creds.api_token.as_str(), "env-token");
    assert!(creds.last_validated_at.is_none());

    assert!(CredentialsBootstrap::is_initialized());
    assert!(CredentialsBootstrap::require().is_ok());
    assert!(
        CredentialsBootstrap::try_init()
            .await
            .expect("try_init after init")
            .is_some()
    );
    assert!(CredentialsBootstrap::expires_within(Duration::hours(1)));

    let err = CredentialsBootstrap::init().await.unwrap_err();
    assert!(err.to_string().contains("already initialized"), "got {err}");

    CredentialsBootstrap::init_empty();
    assert!(CredentialsBootstrap::require().is_ok());
}
