use std::collections::HashMap;

use serde_json::json;
use systemprompt_cloud::deploy::provider::{
    BuiltImage, DeployProvider, DeployProviderKind, SecretBatch, SecretPurpose, SecretsOutcome,
    SecretsTransport, SystempromptCloud,
};
use systemprompt_cloud::{CloudApiClient, CloudError};
use systemprompt_identifiers::TenantId;
use systemprompt_manifest::HostingProvider;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn managed(server: &MockServer) -> SystempromptCloud {
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tenant_bearer",
            "expires_in": 600
        })))
        .mount(server)
        .await;
    let client = CloudApiClient::new(&server.uri(), "operator").unwrap();
    SystempromptCloud::new(client, TenantId::new("t-managed"))
}

#[test]
fn a_docker_local_tenant_has_no_deploy_pipeline() {
    let err = DeployProviderKind::for_provider(HostingProvider::DockerLocal).unwrap_err();
    assert!(matches!(
        err,
        CloudError::ProviderNotDeployable {
            provider: HostingProvider::DockerLocal
        }
    ));
}

#[test]
fn a_secret_batch_reports_sorted_unique_keys_and_hides_values() {
    let batch = SecretBatch::Values {
        purpose: SecretPurpose::VaultBootstrap,
        entries: HashMap::from([
            ("B".to_owned(), "secret-b".to_owned()),
            ("A".to_owned(), "secret-a".to_owned()),
        ]),
    };
    assert_eq!(batch.purpose(), SecretPurpose::VaultBootstrap);
    assert_eq!(batch.keys(), vec!["A".to_owned(), "B".to_owned()]);
    let debug = format!("{batch:?}");
    assert!(!debug.contains("secret-a"), "{debug}");
    let declared = SecretBatch::Declared {
        purpose: SecretPurpose::ProfilePath,
        keys: vec!["X".to_owned(), "X".to_owned()],
    };
    assert_eq!(declared.keys(), vec!["X".to_owned()]);
}

#[tokio::test]
async fn registry_credentials_compose_the_image_and_keep_the_token_out_of_debug() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-managed/registry-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {
            "registry": "registry.fly.io",
            "username": "x",
            "token": "registry-secret",
            "repository": "acme",
            "tag": "v7"
        }})))
        .mount(&server)
        .await;
    let provider = managed(&server).await;
    assert_eq!(provider.tenant_id().as_str(), "t-managed");
    assert_eq!(provider.provider(), HostingProvider::Systemprompt);
    assert_eq!(provider.secrets_transport(), SecretsTransport::Push);

    let target = provider.registry_credentials().await.unwrap();
    assert_eq!(target.image, "registry.fly.io/acme:v7");
    let login = target.login.expect("managed login");
    assert_eq!(login.token, "registry-secret");
    assert!(!format!("{login:?}").contains("registry-secret"));
}

#[tokio::test]
async fn pushed_secret_values_report_their_keys() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-managed/secrets"))
        .and(body_string_contains("JWT_SECRET"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let provider = managed(&server).await;
    let outcome = provider
        .push_secrets(SecretBatch::Values {
            purpose: SecretPurpose::Profile,
            entries: HashMap::from([("JWT_SECRET".to_owned(), "v".to_owned())]),
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        SecretsOutcome::Pushed {
            keys: vec!["JWT_SECRET".to_owned()]
        }
    );
}

#[tokio::test]
async fn declared_secrets_without_values_are_refused_before_any_request() {
    let server = MockServer::start().await;
    let provider = managed(&server).await;
    let err = provider
        .push_secrets(SecretBatch::Declared {
            purpose: SecretPurpose::PlatformCredentials,
            keys: vec!["A".to_owned(), "B".to_owned()],
        })
        .await
        .unwrap_err();
    assert!(
        matches!(&err, CloudError::Deploy { message, .. } if message.contains("A, B")),
        "{err:?}"
    );
}

#[tokio::test]
async fn rollout_reports_the_deploy_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-managed/deploy"))
        .and(body_string_contains("img:1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {
            "status": "deploying",
            "app_url": "https://t.example"
        }})))
        .mount(&server)
        .await;
    let provider = managed(&server).await;
    let report = provider
        .rollout(&BuiltImage {
            reference: "img:1".to_owned(),
            pushed: true,
        })
        .await
        .unwrap();
    assert_eq!(report.status, "deploying");
    assert_eq!(report.app_url.as_deref(), Some("https://t.example"));
    assert!(report.commit.is_none());
}

#[tokio::test]
async fn status_counts_running_states_as_ready() {
    for (state, ready) in [
        ("Running", true),
        ("healthy", true),
        ("provisioning", false),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/tenants/t-managed/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {
                "status": state,
                "app_url": "https://t.example"
            }})))
            .mount(&server)
            .await;
        let health = managed(&server).await.status().await.unwrap();
        assert_eq!(health.ready, ready, "{state}");
        assert_eq!(health.status, state);
        assert_eq!(health.url, "https://t.example");
    }
}
