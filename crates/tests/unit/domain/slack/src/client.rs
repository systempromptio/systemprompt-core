//! Wiremock coverage for the outbound Slack Web API client.
//!
//! `SlackClient::with_base_url` / `with_users_info_url` (the `test` seam) point
//! `chat.postMessage` and `users.info` at a loopback mock so the outbound
//! request, bearer auth, and the `{"ok": false}` logical-failure branch are all
//! observable. The SSRF
//! guard runs before any request, so a blocked host fails closed without a
//! network call.

use serde_json::{Value, json};
use systemprompt_slack::SlackError;
use systemprompt_slack::client::SlackClient;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BLOCKS: fn() -> Value =
    || json!([{ "type": "section", "text": { "type": "mrkdwn", "text": "hi" } }]);

#[tokio::test]
async fn post_message_sends_bearer_and_channel_to_the_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat.postMessage"))
        .and(header("authorization", "Bearer xoxb-test"))
        .and(body_partial_json(json!({ "channel": "C123" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .expect(1)
        .mount(&server)
        .await;

    let url = format!("{}/api/chat.postMessage", server.uri());
    let client = SlackClient::with_base_url(reqwest::Client::new(), "xoxb-test", url);
    client
        .post_message("C123", BLOCKS())
        .await
        .expect("post_message succeeds on ok:true");
}

#[tokio::test]
async fn post_message_surfaces_ok_false_as_outbound_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat.postMessage"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "ok": false, "error": "channel_not_found" })),
        )
        .mount(&server)
        .await;

    let url = format!("{}/api/chat.postMessage", server.uri());
    let client = SlackClient::with_base_url(reqwest::Client::new(), "xoxb-test", url);
    let err = client
        .post_message("C123", BLOCKS())
        .await
        .expect_err("ok:false is an error");
    assert!(
        matches!(err, SlackError::Outbound(ref e) if e == "channel_not_found"),
        "expected Outbound(channel_not_found), got {err:?}"
    );
}

#[tokio::test]
async fn respond_posts_ephemeral_response_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook/ephemeral"))
        .and(body_partial_json(json!({ "response_type": "ephemeral" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .expect(1)
        .mount(&server)
        .await;

    let client = SlackClient::new(reqwest::Client::new(), String::new());
    client
        .respond(&format!("{}/hook/ephemeral", server.uri()), BLOCKS(), true)
        .await
        .expect("ephemeral respond succeeds");
}

#[tokio::test]
async fn respond_posts_in_channel_response_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook/in_channel"))
        .and(body_partial_json(json!({ "response_type": "in_channel" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .expect(1)
        .mount(&server)
        .await;

    let client = SlackClient::new(reqwest::Client::new(), String::new());
    client
        .respond(
            &format!("{}/hook/in_channel", server.uri()),
            BLOCKS(),
            false,
        )
        .await
        .expect("in_channel respond succeeds");
}

#[tokio::test]
async fn respond_rejects_a_blocked_host_before_any_request() {
    let client = SlackClient::new(reqwest::Client::new(), String::new());
    let err = client
        .respond("http://169.254.169.254/hook", BLOCKS(), true)
        .await
        .expect_err("SSRF guard blocks the link-local metadata host");
    assert!(
        matches!(err, SlackError::OutboundUrl(_)),
        "expected OutboundUrl, got {err:?}"
    );
}

#[tokio::test]
async fn post_message_rejects_a_blocked_host_before_any_request() {
    let client = SlackClient::with_base_url(
        reqwest::Client::new(),
        "xoxb-test",
        "http://169.254.169.254/api/chat.postMessage",
    );
    let err = client
        .post_message("C123", BLOCKS())
        .await
        .expect_err("SSRF guard blocks the link-local metadata host");
    assert!(
        matches!(err, SlackError::OutboundUrl(_)),
        "expected OutboundUrl, got {err:?}"
    );
}

#[tokio::test]
async fn non_json_2xx_body_is_treated_as_transport_level_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook/plain"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let client = SlackClient::new(reqwest::Client::new(), String::new());
    client
        .respond(&format!("{}/hook/plain", server.uri()), BLOCKS(), true)
        .await
        .expect("non-JSON 200 falls back to HTTP status success");
}

#[tokio::test]
async fn non_json_5xx_body_surfaces_as_unknown_outbound_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook/broken"))
        .respond_with(ResponseTemplate::new(500).set_body_string("<html>gateway error</html>"))
        .mount(&server)
        .await;

    let client = SlackClient::new(reqwest::Client::new(), String::new());
    let err = client
        .respond(&format!("{}/hook/broken", server.uri()), BLOCKS(), true)
        .await
        .expect_err("non-JSON 5xx is an error");
    assert!(
        matches!(err, SlackError::Outbound(ref e) if e == "unknown"),
        "expected Outbound(unknown), got {err:?}"
    );
}

#[tokio::test]
async fn unreachable_endpoint_surfaces_the_transport_error() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
        .expect("client");
    let slack = SlackClient::with_base_url(client, "xoxb-test", "http://127.0.0.1:9/api");
    let err = slack
        .post_message("C123", BLOCKS())
        .await
        .expect_err("connection to a closed port fails");
    assert!(
        matches!(err, SlackError::Http(_)),
        "expected Http transport error, got {err:?}"
    );
}

fn users_info_client(server: &MockServer) -> SlackClient {
    SlackClient::with_base_url(
        reqwest::Client::new(),
        "xoxb-test",
        "https://slack.com/api/chat.postMessage",
    )
    .with_users_info_url(format!("{}/api/users.info", server.uri()))
}

#[tokio::test]
async fn user_info_reads_a_confirmed_workspace_email() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users.info"))
        .and(header("authorization", "Bearer xoxb-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "user": {
                "id": "U1",
                "is_email_confirmed": true,
                "profile": { "email": "ada@example.com", "real_name": "Ada" }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let profile = users_info_client(&server)
        .user_info("U1")
        .await
        .expect("users.info succeeds on ok:true");
    assert_eq!(profile.email.as_deref(), Some("ada@example.com"));
    assert!(profile.email_confirmed);
    assert_eq!(profile.display_name.as_deref(), Some("Ada"));
}

#[tokio::test]
async fn user_info_reports_an_unconfirmed_email_as_unconfirmed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users.info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "user": { "id": "U1", "profile": { "email": "ada@example.com" } }
        })))
        .mount(&server)
        .await;

    let profile = users_info_client(&server)
        .user_info("U1")
        .await
        .expect("a missing is_email_confirmed is not an error");
    assert!(
        !profile.email_confirmed,
        "an absent confirmation flag must never read as confirmed"
    );
}

#[tokio::test]
async fn user_info_surfaces_ok_false_as_outbound_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users.info"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "ok": false, "error": "missing_scope" })),
        )
        .mount(&server)
        .await;

    let err = users_info_client(&server)
        .user_info("U1")
        .await
        .expect_err("ok:false is an error");
    assert!(
        matches!(err, SlackError::Outbound(ref e) if e == "missing_scope"),
        "expected Outbound(missing_scope), got {err:?}"
    );
}

#[tokio::test]
async fn user_info_rejects_a_blocked_host_before_any_request() {
    let client = SlackClient::with_base_url(
        reqwest::Client::new(),
        "xoxb-test",
        "https://slack.com/api/chat.postMessage",
    )
    .with_users_info_url("http://169.254.169.254/api/users.info");
    let err = client
        .user_info("U1")
        .await
        .expect_err("SSRF guard blocks the link-local metadata host");
    assert!(
        matches!(err, SlackError::OutboundUrl(_)),
        "expected OutboundUrl, got {err:?}"
    );
}
