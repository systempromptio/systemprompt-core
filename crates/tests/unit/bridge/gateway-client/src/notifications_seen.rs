use systemprompt_bridge::gateway::{GatewayClient, GatewayError};
use systemprompt_bridge::ids::BearerToken;
use systemprompt_identifiers::{NotificationId, ValidatedUrl};
use systemprompt_models::ApiPaths;
use systemprompt_models::notifications::MarkSeen;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ID: &str = "00000000-0000-4000-8000-0000000000a1";

fn client(server: &MockServer) -> GatewayClient {
    GatewayClient::new(
        ValidatedUrl::try_new(server.uri()).expect("valid ValidatedUrl"),
        reqwest::Client::new(),
    )
}

fn body() -> MarkSeen {
    MarkSeen {
        ids: vec![NotificationId::new(ID)],
    }
}

#[tokio::test]
async fn seen_posts_the_shown_ids_to_the_bridge_seen_route() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_NOTIFICATIONS_SEEN))
        .and(header("authorization", "Bearer seen-bearer"))
        .and(body_json(serde_json::json!({ "ids": [ID] })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client(&server)
        .mark_notifications_seen(&BearerToken::new("seen-bearer"), &body())
        .await
        .expect("seen accepted");
}

#[tokio::test]
async fn a_refused_seen_report_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_NOTIFICATIONS_SEEN))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = client(&server)
        .mark_notifications_seen(&BearerToken::new("seen-bearer"), &body())
        .await
        .expect_err("a 500 is not a recorded receipt");
    assert!(matches!(err, GatewayError::Rejected { .. }), "{err:?}");
}
