//! Tool history as the next request sends it: every stored call is followed
//! by a result, a call that never finished gets a placeholder, results that
//! exist are kept as they are, and each result answers the call whose id it
//! carries.

use serde_json::{Value, json};
use systemprompt_engine::store::{FinishReason, JsonMap, ToolState, now_ms};

use crate::request_mapping_tests::seed::{Seed, text, tool};
use crate::request_mapping_tests::{ask, text_block, user_turn};
use crate::support::{Scripted, text_turn};

const INTERRUPTED: &str = "The tool call was interrupted before it finished.";

fn done(path: &str, output: &str) -> ToolState {
    ToolState::Completed {
        input: json!({"path": path}),
        output: output.to_owned(),
        title: path.to_owned(),
        metadata: JsonMap::new(),
        attachments: Vec::new(),
        started: now_ms(),
        ended: now_ms(),
    }
}

fn pending(path: &str) -> ToolState {
    ToolState::Pending {
        input: json!({"path": path}),
    }
}

fn running(path: &str) -> ToolState {
    ToolState::Running {
        input: json!({"path": path}),
        title: None,
        metadata: JsonMap::new(),
        started: now_ms(),
    }
}

// JSON: a Messages tool_use block as the instance recorded it.
fn call(id: &str, path: &str) -> Value {
    json!({"type": "tool_use", "id": id, "name": "read", "input": {"path": path}})
}

// JSON: a Messages tool_result block holding plain text.
fn result(id: &str, output: &str, is_error: bool) -> Value {
    let mut block = json!({"type": "tool_result", "tool_use_id": id, "content": output});
    if is_error {
        block["is_error"] = json!(true);
    }
    block
}

/// A session whose last reply asked for these calls, then the turns of the
/// next request.
// JSON: the `messages` array of the next request body.
async fn next_request(calls: Vec<(&str, ToolState)>) -> Value {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let id = scripted.session().await;
    let mut seed = Seed::new(&scripted.engine, &id);
    seed.user(vec![text("Read them")]);
    seed.reply_with(
        |reply| reply.finish = Some(FinishReason::ToolCalls),
        calls
            .into_iter()
            .map(|(call, state)| tool(call, "read", state))
            .collect(),
    );
    let messages = ask(&scripted, &id, "Go on").await;
    scripted.finish().await;
    messages
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_left_without_a_result_is_answered_with_a_placeholder_before_the_next_step() {
    let messages = next_request(vec![("call_1", pending("a.txt"))]).await;
    assert_eq!(
        messages,
        json!([
            user_turn(&["Read them"]),
            {"role": "assistant", "content": [call("call_1", "a.txt")]},
            {"role": "user", "content": [
                result("call_1", INTERRUPTED, true),
                text_block("Go on")
            ]}
        ])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn trailing_unfinished_calls_get_placeholders_and_finished_results_are_kept() {
    let messages = next_request(vec![
        ("call_1", done("a.txt", "alpha")),
        ("call_2", running("b.txt")),
        ("call_3", pending("c.txt")),
    ])
    .await;
    assert_eq!(
        messages[2]["content"],
        json!([
            result("call_1", "alpha", false),
            result("call_2", INTERRUPTED, true),
            result("call_3", INTERRUPTED, true),
            text_block("Go on")
        ])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn whitespace_results_are_sent_unchanged() {
    let messages = next_request(vec![("call_1", done("a.txt", "  \n"))]).await;
    assert_eq!(messages[2]["content"][0], result("call_1", "  \n", false));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an empty tool result is sent as empty content rather than given a placeholder"]
async fn an_empty_result_is_sent_with_a_placeholder_text() {
    let messages = next_request(vec![("call_1", done("a.txt", ""))]).await;
    let content = &messages[2]["content"][0]["content"];
    assert!(
        content.as_str().is_some_and(|text| !text.is_empty()),
        "{content}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn each_result_answers_the_call_whose_id_it_carries() {
    let messages = next_request(vec![
        ("call_1", done("same.txt", "first")),
        ("call_2", done("same.txt", "second")),
    ])
    .await;
    assert_eq!(
        messages[1]["content"],
        json!([call("call_1", "same.txt"), call("call_2", "same.txt")])
    );
    assert_eq!(
        messages[2]["content"],
        json!([
            result("call_1", "first", false),
            result("call_2", "second", false),
            text_block("Go on")
        ])
    );
}
