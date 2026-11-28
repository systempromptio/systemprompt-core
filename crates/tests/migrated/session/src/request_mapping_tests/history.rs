//! Stored history replayed into the next request: which replies are sent back,
//! what a tool call and its result become, and what stands in for the history a
//! compaction replaced.

use serde_json::json;
use systemprompt_engine::prompt::compaction::summary_message;
use systemprompt_engine::store::{
    Attachment, FinishReason, JsonMap, MessageError, MessageErrorKind, PartBody, ToolState, Usage,
    now_ms,
};

use super::seed::{Seed, reasoning, text, tool};
use super::vision::Vision;
use super::{ask, block_types, image_block, text_block, user_turn};
use crate::support::{Scripted, settle, text_turn, tool_turn};

fn failure(kind: MessageErrorKind, message: &str) -> MessageError {
    MessageError {
        kind,
        message: message.to_owned(),
        status: None,
        retryable: false,
    }
}

fn read_done(output: &str, attachments: Vec<Attachment>) -> ToolState {
    ToolState::Completed {
        input: json!({"path": "notes.txt"}),
        output: output.to_owned(),
        title: "notes.txt".to_owned(),
        metadata: JsonMap::new(),
        attachments,
        started: now_ms(),
        ended: now_ms(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn replies_with_nothing_to_send_are_left_out_and_signature_only_reasoning_is_kept() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let id = scripted.session().await;
    let mut seed = Seed::new(&scripted.engine, &id);
    seed.user(vec![text("first")]);
    seed.reply(Vec::new());
    seed.reply(vec![text("")]);
    seed.reply(vec![reasoning("", None)]);
    seed.reply(vec![text("Partial")]);
    seed.reply(vec![reasoning("", Some("sig_1"))]);

    let messages = ask(&scripted, &id, "next").await;
    assert_eq!(
        messages,
        json!([
            user_turn(&["first"]),
            {"role": "assistant", "content": [
                text_block("Partial"),
                {"type": "thinking", "thinking": "", "signature": "sig_1"}
            ]},
            user_turn(&["next"])
        ])
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn each_kind_of_stored_message_takes_its_place_in_the_request() {
    let vision = Vision::start(vec![text_turn("ok")]).await;
    let id = vision.session().await;
    let mut seed = Seed::new(&vision.engine, &id);
    seed.user(vec![text("ancient question")]);
    seed.reply(vec![text("ancient answer")]);
    seed.user(vec![PartBody::Compaction { auto: false }]);
    seed.reply_with(|summary| summary.system = true, vec![text("Earlier work")]);
    seed.engine_user(vec![text("Synthetic context")]);
    seed.user(vec![
        text("Inspect this image"),
        PartBody::File {
            mime: "image/png".to_owned(),
            name: Some("hello.png".to_owned()),
            url: "data:image/png;base64,aGVsbG8=".to_owned(),
        },
    ]);
    seed.reply(vec![
        PartBody::StepStart { snapshot: None },
        text("Looking."),
        PartBody::StepFinish {
            reason: FinishReason::Stop,
            usage: Usage::default(),
            snapshot: None,
        },
        PartBody::Patch {
            hash: "0a1b2c".to_owned(),
            files: vec!["notes.txt".to_owned()],
        },
    ]);

    let messages = vision.ask(&id, "What next?", Vec::new()).await;
    assert_eq!(
        messages,
        json!([
            {"role": "user", "content": [
                text_block(&summary_message("Earlier work")),
                text_block("Synthetic context"),
                text_block("Inspect this image"),
                image_block("image/png", "aGVsbG8=")
            ]},
            {"role": "assistant", "content": [text_block("Looking.")]},
            user_turn(&["What next?"])
        ])
    );
    vision.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the summary frame holds the summary alone; subagent sessions and background work are not listed after it"]
async fn a_compaction_summary_is_followed_by_the_sessions_subagents() {
    let delegate = tool_turn(
        "task1",
        "task",
        json!({"description": "look around", "prompt": "Find the notes file", "agent": "general"}),
    );
    let scripted = Scripted::start(vec![
        delegate,
        text_turn("child report"),
        text_turn("parent done"),
        text_turn("SUMMARY of the work"),
        text_turn("after"),
    ])
    .await;
    let id = scripted.session().await;
    ask(&scripted, &id, "Delegate").await;
    let mut events = scripted.engine.subscribe();
    scripted.engine.compact(&id).await.unwrap();
    settle(&mut events, &id).await;

    let messages = ask(&scripted, &id, "next").await;
    let children = scripted.engine.store().child_sessions(&id).unwrap();
    assert_eq!(children.len(), 1);
    let opening = messages[0]["content"].to_string();
    assert!(opening.contains("SUMMARY of the work"), "{opening}");
    assert!(opening.contains(&children[0].id.to_string()), "{opening}");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tool_call_with_media_is_replayed_as_a_call_and_a_result_holding_the_media() {
    let vision = Vision::start(vec![text_turn("ok")]).await;
    let id = vision.session().await;
    let mut seed = Seed::new(&vision.engine, &id);
    seed.user(vec![text("Read it")]);
    let picture = Attachment {
        mime: "image/png".to_owned(),
        name: Some("hello.png".to_owned()),
        url: "data:image/png;base64,aGVsbG8=".to_owned(),
    };
    let unfinished = ToolState::Pending {
        input: json!({"path": "notes.txt"}),
    };
    seed.reply_with(
        |reply| reply.finish = Some(FinishReason::ToolCalls),
        vec![
            reasoning("Think", Some("sig_1")),
            text("Checking"),
            tool("call_pending", "read", unfinished),
            tool("call_done", "read", read_done("Hello", vec![picture])),
        ],
    );

    let messages = vision.ask(&id, "Go on", Vec::new()).await;
    assert_eq!(messages[0], user_turn(&["Read it"]));
    assert_eq!(
        messages[1],
        json!({"role": "assistant", "content": [
            {"type": "thinking", "thinking": "Think", "signature": "sig_1"},
            text_block("Checking"),
            {"type": "tool_use", "id": "call_pending", "name": "read", "input": {"path": "notes.txt"}},
            {"type": "tool_use", "id": "call_done", "name": "read", "input": {"path": "notes.txt"}}
        ]})
    );
    let results = &messages[2]["content"];
    assert_eq!(results[0]["tool_use_id"], "call_pending");
    assert_eq!(results[0]["is_error"], true);
    assert_eq!(
        results[1],
        json!({"type": "tool_result", "tool_use_id": "call_done", "content": [
            text_block("Hello"),
            image_block("image/png", "aGVsbG8=")
        ]})
    );
    assert_eq!(results[2], text_block("Go on"));
    assert_eq!(messages.as_array().map(Vec::len), Some(3));
    vision.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_reply_is_replayed_only_up_to_its_last_tool_call_that_ran() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let id = scripted.session().await;
    let mut seed = Seed::new(&scripted.engine, &id);
    let dropped = |reply: &mut systemprompt_engine::store::AssistantMessage| {
        reply.finish = Some(FinishReason::Error);
        reply.error = Some(failure(MessageErrorKind::Api, "the stream dropped"));
    };
    seed.user(vec![text("Start")]);
    seed.reply_with(dropped, vec![text("Partial visible answer")]);
    seed.reply_with(
        dropped,
        vec![
            text("Reading first"),
            tool("call_ran", "read", read_done("Hello", Vec::new())),
            text("Now I will run the te"),
        ],
    );

    let messages = ask(&scripted, &id, "Try again").await;
    assert_eq!(
        messages[1],
        json!({"role": "assistant", "content": [
            text_block("Reading first"),
            {"type": "tool_use", "id": "call_ran", "name": "read", "input": {"path": "notes.txt"}}
        ]})
    );
    assert_eq!(block_types(&messages[2]), ["tool_result", "text"]);
    let sent = messages.to_string();
    assert!(!sent.contains("Partial visible answer"), "{sent}");
    assert!(!sent.contains("run the te"), "{sent}");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an interrupted reply that holds only signed reasoning is sent back as a turn of its own"]
async fn an_interrupted_reply_that_holds_only_reasoning_is_left_out() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let id = scripted.session().await;
    let mut seed = Seed::new(&scripted.engine, &id);
    seed.user(vec![text("Think about it")]);
    seed.reply_with(
        |reply| {
            reply.finish = None;
            reply.completed_at = None;
        },
        vec![
            reasoning("Completed summary part", Some("sig_done")),
            reasoning("", None),
        ],
    );

    let messages = ask(&scripted, &id, "Again").await;
    assert_eq!(messages, json!([user_turn(&["Think about it", "Again"])]));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_aborted_reply_keeps_its_signed_reasoning_and_visible_text_in_the_next_request() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let id = scripted.session().await;
    let mut seed = Seed::new(&scripted.engine, &id);
    let aborted = |reply: &mut systemprompt_engine::store::AssistantMessage| {
        reply.finish = Some(FinishReason::Aborted);
        reply.error = Some(failure(MessageErrorKind::Aborted, "the run was aborted"));
    };
    seed.user(vec![text("Go")]);
    seed.reply_with(
        aborted,
        vec![
            reasoning("Finished thought", Some("sig_kept")),
            text("Closed commentary."),
        ],
    );
    seed.reply_with(aborted, vec![text("Cut off sentence")]);

    let messages = ask(&scripted, &id, "Continue").await;
    assert_eq!(
        messages,
        json!([
            user_turn(&["Go"]),
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "Finished thought", "signature": "sig_kept"},
                text_block("Closed commentary."),
                text_block("Cut off sentence")
            ]},
            user_turn(&["Continue"])
        ])
    );
    scripted.finish().await;
}
