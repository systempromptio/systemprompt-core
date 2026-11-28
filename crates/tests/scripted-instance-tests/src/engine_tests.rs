use std::time::Duration;

use serde_json::Value;
use systemprompt_harness::command::{
    Answers, EventWriter, ExitStatus, PermissionMode, RunRequest, run,
};
use systemprompt_scripted_instance::{Expect, ModelScript, Respond, StopReason, Turn, Usage};
use systemprompt_traits::CancellationToken;

use crate::fixture::Signed;

const ANSWER: &str = "ok";

#[tokio::test(flavor = "multi_thread")]
async fn one_prompt_is_answered_through_the_governed_engine() {
    let script = ModelScript::from_turns([Turn {
        expect: Expect::default(),
        respond: vec![Respond::Text(ANSWER.to_owned())],
        stop: Some(StopReason::End),
        usage: Some(Usage {
            input: 7,
            output: 3,
        }),
        fault: None,
    }]);
    let signed = Signed::start(script).await;
    let mut out = EventWriter::new(Vec::new());
    let request = RunRequest {
        prompts: vec!["Reply with the single word: ok".to_owned()],
        workspace: signed.workspace(),
        permission_mode: PermissionMode::Deny,
        answers: Answers::parse("{}").unwrap(),
        model: None,
        agent: None,
        data_root: Some(signed.root.clone()),
        sync_timeout: Duration::from_secs(30),
        timeout: Some(Duration::from_secs(60)),
    };
    let report = run(&signed.host, request, &CancellationToken::new(), &mut out).await;
    assert_eq!(report.status, ExitStatus::Completed, "{report:?}");

    let events: Vec<Value> = String::from_utf8(out.into_inner())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let replies: Vec<&Value> = events
        .iter()
        .filter(|event| event["type"] == "message" && event["message"]["role"] == "assistant")
        .map(|event| &event["message"])
        .collect();
    let last = replies.last().expect("no assistant message was stored");
    assert!(
        last.get("error").is_none_or(Value::is_null),
        "the step failed: {last}"
    );
    assert_eq!(last["usage"]["output"], 3);
    let text: String = events
        .iter()
        .filter(|event| event["type"] == "part" && event["part"]["body"]["type"] == "text")
        .filter(|event| event["part"]["message_id"] == last["id"])
        .filter_map(|event| event["part"]["body"]["text"].as_str())
        .next_back()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(text, ANSWER);
    assert_eq!(signed.instance.unconsumed_turns(), 0);
    assert!(signed.instance.unscripted_requests().is_empty());
    assert_eq!(signed.relay.title_calls(), 1);
    signed.accounts.shutdown();
}
