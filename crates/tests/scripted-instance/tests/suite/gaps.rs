use std::time::{Duration, Instant};

use serde_json::{Value, json};
use systemprompt_scripted_instance::{
    Aside, ModelFixture, ModelScript, Respond, ScriptedInstance, StopReason, Turn, TurnFault,
};
use systemprompt_wire::WireProtocol;
use systemprompt_wire::canonical::CanonicalStopReason;

use crate::support::{PROTOCOLS, decode, instance, request, send};

const TITLE: &str = "A scripted title";

fn step() -> Turn {
    Turn {
        respond: vec![Respond::Text("the step".to_owned())],
        ..Turn::default()
    }
}

async fn titled(script: ModelScript) -> ScriptedInstance {
    ScriptedInstance::builder()
        .models([ModelFixture::messages("m-a"), ModelFixture::chat("m-b")])
        .script(script)
        .title_reply(TITLE)
        .start()
        .await
        .unwrap()
}

fn with_reply(protocol: WireProtocol, reply: &str) -> Value {
    let mut body = request(protocol, &["read"], &[]);
    body["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "role": "assistant", "content": reply }));
    body["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "role": "user", "content": "again" }));
    body
}

#[tokio::test]
async fn a_title_call_is_answered_by_the_title_reply_and_takes_no_turn() {
    for protocol in PROTOCOLS {
        let instance = titled(ModelScript::from_turns([step()])).await;

        let title = send(&instance, protocol, &request(protocol, &[], &[])).await;
        let title = decode(protocol, title).await;
        assert_eq!(title.text, TITLE);
        assert_eq!(instance.unconsumed_turns(), 1);

        let answer = send(&instance, protocol, &with_reply(protocol, "hello")).await;
        assert_eq!(decode(protocol, answer).await.text, "the step");
        assert_eq!(instance.unconsumed_turns(), 0);
        assert!(instance.unscripted_requests().is_empty());
        let purposes: Vec<Option<String>> = instance
            .requests()
            .into_iter()
            .map(|record| record.purpose)
            .collect();
        assert_eq!(purposes, [Some(Aside::TITLE.to_owned()), None]);
    }
}

#[tokio::test]
async fn a_request_with_tools_or_history_is_never_a_title_call() {
    let protocol = WireProtocol::Anthropic;
    let instance = titled(ModelScript::empty()).await;
    let with_tools = send(&instance, protocol, &request(protocol, &["read"], &[])).await;
    assert_eq!(with_tools.status().as_u16(), 400);
    let mut history = with_reply(protocol, "hello");
    history["tools"] = json!([]);
    let with_history = send(&instance, protocol, &history).await;
    assert_eq!(with_history.status().as_u16(), 400);
    assert_eq!(instance.unscripted_requests().len(), 2);
}

#[tokio::test]
async fn without_a_title_reply_a_title_call_takes_the_next_turn() {
    let protocol = WireProtocol::Anthropic;
    let instance = instance(ModelScript::from_turns([step()])).await;
    let answer = send(&instance, protocol, &request(protocol, &[], &[])).await;
    assert_eq!(decode(protocol, answer).await.text, "the step");
    assert_eq!(instance.requests()[0].purpose, None);
}

#[tokio::test]
async fn the_output_limit_and_refusal_stops_decode_as_their_reasons() {
    for protocol in PROTOCOLS {
        for (stop, expected) in [
            (StopReason::MaxTokens, CanonicalStopReason::MaxTokens),
            (StopReason::ContentFilter, CanonicalStopReason::Refusal),
        ] {
            let turn = Turn {
                stop: Some(stop),
                ..step()
            };
            let instance = instance(ModelScript::from_turns([turn])).await;
            let answer = send(&instance, protocol, &request(protocol, &[], &[])).await;
            let decoded = decode(protocol, answer).await;
            assert_eq!(decoded.text, "the step");
            assert_eq!(decoded.stop, Some(expected), "{protocol:?} {stop:?}");
        }
    }
}

#[test]
fn the_new_stops_and_faults_read_from_their_scenario_spelling() {
    let script = ModelScript::from_json(
        r#"{"turns": [{"stop": "max_tokens"}, {"stop": "content_filter"},
                      {"fault": "context_overflow"}, {"fault": {"headers_after_ms": 50}}]}"#,
    )
    .unwrap();
    assert_eq!(script.turns[0].stop, Some(StopReason::MaxTokens));
    assert_eq!(script.turns[1].stop, Some(StopReason::ContentFilter));
    assert_eq!(script.turns[2].fault, Some(TurnFault::ContextOverflow));
    assert_eq!(script.turns[3].fault, Some(TurnFault::HeadersAfterMs(50)));
}

#[tokio::test]
async fn a_context_overflow_answers_with_each_providers_overflow_body() {
    for protocol in PROTOCOLS {
        let turn = Turn {
            fault: Some(TurnFault::ContextOverflow),
            ..step()
        };
        let instance = instance(ModelScript::from_turns([turn])).await;
        let answer = send(&instance, protocol, &request(protocol, &[], &[])).await;
        assert_eq!(answer.status().as_u16(), 400);
        let body: Value = answer.json().await.unwrap();
        let message = body["error"]["message"].as_str().unwrap();
        match protocol {
            WireProtocol::OpenAiChat => {
                assert_eq!(body["error"]["code"], "context_length_exceeded");
                assert!(message.contains("maximum context length is 200000 tokens"));
            },
            _ => assert!(message.starts_with("prompt is too long:")),
        }
        assert_eq!(instance.unconsumed_turns(), 0);
        assert!(instance.unscripted_requests().is_empty());
    }
}

#[tokio::test]
async fn held_headers_delay_the_whole_answer_then_answer_the_turn() {
    for protocol in PROTOCOLS {
        let turn = Turn {
            fault: Some(TurnFault::HeadersAfterMs(300)),
            ..step()
        };
        let instance = instance(ModelScript::from_turns([turn])).await;
        let began = Instant::now();
        let answer = send(&instance, protocol, &request(protocol, &[], &[])).await;
        assert!(began.elapsed() >= Duration::from_millis(300));
        assert_eq!(decode(protocol, answer).await.text, "the step");
    }
}

#[tokio::test]
async fn a_client_that_gives_up_on_held_headers_times_out() {
    let protocol = WireProtocol::Anthropic;
    let turn = Turn {
        fault: Some(TurnFault::HeadersAfterMs(5_000)),
        ..step()
    };
    let instance = instance(ModelScript::from_turns([turn])).await;
    let refused = reqwest::Client::new()
        .post(format!(
            "{}{}",
            instance.base_url(),
            crate::support::path(protocol)
        ))
        .bearer_auth(instance.pat().as_str())
        .timeout(Duration::from_millis(200))
        .json(&request(protocol, &[], &[]))
        .send()
        .await
        .unwrap_err();
    assert!(refused.is_timeout());
    assert_eq!(instance.unconsumed_turns(), 0);
}
