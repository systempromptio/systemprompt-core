//! Wiremock coverage for the outbound Slack Web API client.
//!
//! `SlackClient::with_base_url` (the `test` seam) points
//! `chat.postMessage` at a loopback mock so the outbound request, bearer auth,
//! and the `{"ok": false}` logical-failure branch are all observable. The SSRF
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
