//! Wiremock coverage for the outbound Bot Connector reply.
//!
//! `TeamsClient::with_endpoints` (the `test` seam) redirects token
//! acquisition to a loopback mock; the reply target is the activity's
//! `serviceUrl`, here the same mock. One server serves both `/token` and the
//! Bot Connector activities endpoint so the full reply path is observable.

use systemprompt_identifiers::TeamsConversationId;
use systemprompt_teams::TeamsError;
use systemprompt_teams::client::TeamsClient;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn mount_token(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "bot-token",
            "expires_in": 3600,
        })))
        .mount(server)
        .await;
}

fn client(server: &MockServer) -> TeamsClient {
    TeamsClient::with_endpoints(
        reqwest::Client::new(),
        "app-1",
        "secret",
        format!("{}/token", server.uri()),
    )
}

fn conversation() -> TeamsConversationId {
    TeamsConversationId::new("19:abc@thread.v2")
}

#[tokio::test]
async fn reply_posts_the_card_to_the_bot_connector() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    Mock::given(method("POST"))
        .and(path("/v3/conversations/19:abc@thread.v2/activities"))
        .and(body_partial_json(serde_json::json!({ "type": "message" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({ "id": "1" })))
        .expect(1)
        .mount(&server)
        .await;

    let attachments = systemprompt_teams::cards::render_card("hello");
    client(&server)
        .reply(&server.uri(), &conversation(), attachments, 0)
        .await
        .expect("reply succeeds on 2xx");
}

#[tokio::test]
async fn non_2xx_reply_is_an_outbound_error() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    Mock::given(method("POST"))
        .and(path("/v3/conversations/19:abc@thread.v2/activities"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&server)
        .await;

    let attachments = systemprompt_teams::cards::render_card("hello");
    let err = client(&server)
        .reply(&server.uri(), &conversation(), attachments, 0)
        .await
        .expect_err("a 403 from the Bot Connector surfaces");
    assert!(
        matches!(err, TeamsError::Outbound(_)),
        "expected Outbound, got {err:?}"
    );
}

#[tokio::test]
async fn production_constructor_wires_the_default_endpoints() {
    // `TeamsClient::new` builds a `TokenProvider` against the hardcoded Bot
    // Framework login authority. Driving a blocked `serviceUrl` exercises that
    // production path through `reply`'s SSRF guard, which fails closed before any
    // token acquisition — so no live Bot Connector call is made.
    let client = TeamsClient::new(reqwest::Client::new(), "app-1", "secret");
    let attachments = systemprompt_teams::cards::render_card("hello");
    let err = client
        .reply("http://169.254.169.254", &conversation(), attachments, 0)
        .await
        .expect_err("the SSRF guard blocks the link-local metadata host");
    assert!(
        matches!(err, TeamsError::OutboundUrl(_)),
        "expected OutboundUrl, got {err:?}"
    );
}

#[tokio::test]
async fn reply_rejects_a_blocked_service_url_before_any_request() {
    let server = MockServer::start().await;
    mount_token(&server).await;

    let attachments = systemprompt_teams::cards::render_card("hello");
    let err = client(&server)
        .reply("http://169.254.169.254", &conversation(), attachments, 0)
        .await
        .expect_err("SSRF guard blocks the link-local metadata host");
    assert!(
        matches!(err, TeamsError::OutboundUrl(_)),
        "expected OutboundUrl, got {err:?}"
    );
}
