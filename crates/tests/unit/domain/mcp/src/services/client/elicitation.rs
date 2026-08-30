//! Elicitation routing when a server asks the client for user input.
//!
//! An elicitation is a consent prompt: the server pauses a call and asks a
//! human to supply something. The client only advertises the capability when a
//! delegate is installed, but a server may send the request anyway — and the
//! answer then has to be `Decline`. Returning `Accept` would tell the server
//! someone agreed to something nobody was asked, and returning an error would
//! leave the round hanging rather than closing it on both sides.

use std::sync::Arc;

use rmcp::model::{ElicitRequestParams, ElicitResult, ElicitationAction, ElicitationSchema};
use systemprompt_mcp::services::client::{ElicitationDelegate, SharedElicitationDelegate};
use systemprompt_mcp::test_api::handle_elicitation;

#[derive(Debug)]
struct RecordingDelegate {
    answer: ElicitationAction,
    seen: std::sync::Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl ElicitationDelegate for RecordingDelegate {
    async fn elicit(&self, params: ElicitRequestParams) -> ElicitResult {
        let message = match &params {
            ElicitRequestParams::FormElicitationParams { message, .. }
            | ElicitRequestParams::UrlElicitationParams { message, .. } => message.clone(),
            _ => "unknown".to_owned(),
        };
        self.seen.lock().expect("lock").push(message);

        ElicitResult::new(self.answer.clone()).with_content(serde_json::json!({"answered": true}))
    }
}

fn form_request(message: &str) -> ElicitRequestParams {
    ElicitRequestParams::FormElicitationParams {
        meta: None,
        message: message.to_owned(),
        requested_schema: ElicitationSchema::new(std::collections::BTreeMap::new()),
    }
}

fn url_request(message: &str) -> ElicitRequestParams {
    ElicitRequestParams::UrlElicitationParams {
        meta: None,
        message: message.to_owned(),
        url: "https://approve.invalid/consent".to_owned(),
        elicitation_id: "elicit-1".to_owned(),
    }
}

// Why: this is the consent property. With no delegate there is nobody to ask,
// so the only honest answer is a decline — `Accept` would report agreement
// that was never sought.
#[tokio::test]
async fn a_request_with_no_delegate_is_declined() {
    let result = handle_elicitation(None, form_request("share your address?")).await;

    assert_eq!(
        result.action,
        ElicitationAction::Decline,
        "with nobody to ask, the request must be declined"
    );
    assert!(
        result.content.is_none(),
        "a decline carries no user-supplied content: {:?}",
        result.content
    );
}

// Why: the URL mode is the in-person approval flow. It must decline on the
// same terms — a mode that fell through to a default would approve an
// out-of-band consent step nobody completed.
#[tokio::test]
async fn a_url_mode_request_with_no_delegate_is_also_declined() {
    let result = handle_elicitation(None, url_request("approve on your device")).await;

    assert_eq!(result.action, ElicitationAction::Decline);
    assert!(result.content.is_none());
}

// Why: with a delegate installed the client is claiming a human answered. The
// delegate's decision must be returned as given, not reinterpreted.
#[tokio::test]
async fn a_delegates_answer_is_returned_unchanged() {
    for answer in [
        ElicitationAction::Accept,
        ElicitationAction::Decline,
        ElicitationAction::Cancel,
    ] {
        let delegate: SharedElicitationDelegate = Arc::new(RecordingDelegate {
            answer: answer.clone(),
            seen: std::sync::Mutex::new(Vec::new()),
        });

        let result = handle_elicitation(Some(&delegate), form_request("ok?")).await;

        assert_eq!(
            result.action, answer,
            "the delegate's decision must reach the server as given"
        );
    }
}

// Why: the delegate is what shows the prompt to a human. Handing it a
// different request than the server sent would put one question on screen and
// answer another.
#[tokio::test]
async fn the_delegate_receives_the_request_the_server_sent() {
    let recorder = Arc::new(RecordingDelegate {
        answer: ElicitationAction::Accept,
        seen: std::sync::Mutex::new(Vec::new()),
    });
    let delegate: SharedElicitationDelegate = recorder.clone();

    handle_elicitation(Some(&delegate), form_request("delete everything?")).await;

    assert_eq!(
        recorder.seen.lock().expect("lock").as_slice(),
        ["delete everything?"],
        "the delegate must be shown the server's own message"
    );
}

#[tokio::test]
async fn an_accepting_delegates_content_reaches_the_server() {
    let delegate: SharedElicitationDelegate = Arc::new(RecordingDelegate {
        answer: ElicitationAction::Accept,
        seen: std::sync::Mutex::new(Vec::new()),
    });

    let result = handle_elicitation(Some(&delegate), form_request("name?")).await;

    assert_eq!(result.action, ElicitationAction::Accept);
    assert_eq!(
        result.content,
        Some(serde_json::json!({"answered": true})),
        "what the user supplied must not be dropped on the way back"
    );
}
