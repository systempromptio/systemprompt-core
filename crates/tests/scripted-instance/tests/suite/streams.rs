use serde_json::json;
use systemprompt_models::ApiPaths;
use systemprompt_scripted_instance::{Event, ModelScript, Route, ScriptedInstance};
use systemprompt_wire::canonical::{CanonicalContent, CanonicalStopReason};
use systemprompt_wire::{WireProtocol, anthropic, openai_chat};

use crate::support::{PROTOCOLS, decode, instance, request, send};

const SCRIPT: &str = r#"{
  "turns": [
    {
      "expect": { "tools_include": ["read", "edit"] },
      "respond": [
        { "text": "I will read the file first." },
        { "tool_call": { "id": "t1", "name": "read", "input": { "path": "src/lib.rs" },
                         "argument_chunks": ["{\"pa", "th\": \"src/", "lib.rs\"}"] } }
      ],
      "stop": "tool_use",
      "usage": { "input": 120, "output": 18 }
    },
    {
      "expect": { "tool_results": ["t1"] },
      "respond": [ { "text": "Done." } ],
      "stop": "end"
    }
  ]
}"#;

async fn scripted() -> (ScriptedInstance, ModelScript) {
    let script = ModelScript::from_json(SCRIPT).unwrap();
    (instance(script.clone()).await, script)
}

#[tokio::test]
async fn one_script_decodes_to_its_turns_on_both_protocols() {
    for protocol in PROTOCOLS {
        let (instance, script) = scripted().await;
        assert_eq!(instance.unconsumed_turns(), 2);

        let first = send(
            &instance,
            protocol,
            &request(protocol, &["edit", "read", "bash"], &[]),
        )
        .await;
        assert_eq!(first.status().as_u16(), 200);
        assert_eq!(first.headers()["content-type"], "text/event-stream");
        let first = decode(protocol, first).await;
        first.assert_is(&script.turns[0]);
        assert_eq!(first.stop, Some(CanonicalStopReason::ToolUse));
        assert_eq!(
            first.tools[0].chunks,
            ["{\"pa", "th\": \"src/", "lib.rs\"}"]
        );

        let second = send(
            &instance,
            protocol,
            &request(protocol, &["read", "edit"], &["t1"]),
        )
        .await;
        let second = decode(protocol, second).await;
        second.assert_is(&script.turns[1]);
        assert_eq!(second.stop, Some(CanonicalStopReason::EndTurn));

        assert_eq!(instance.unconsumed_turns(), 0);
        assert!(instance.unscripted_requests().is_empty());
        let call = instance.wait_for(Event::ModelCall).await.unwrap();
        let route = match protocol {
            WireProtocol::OpenAiChat => Route::ChatCompletions,
            _ => Route::Messages,
        };
        assert_eq!((call.route, call.sequence), (route, 0));
    }
}

#[tokio::test]
async fn a_buffered_request_gets_the_turn_as_one_body() {
    for protocol in PROTOCOLS {
        let (instance, _) = scripted().await;
        let mut body = request(protocol, &["read", "edit"], &[]);
        body["stream"] = json!(false);
        let answer: serde_json::Value =
            send(&instance, protocol, &body).await.json().await.unwrap();
        let parsed = match protocol {
            WireProtocol::OpenAiChat => openai_chat::parse_response(&answer, "none"),
            _ => anthropic::parse_response(&answer, "none"),
        }
        .unwrap();
        assert_eq!(parsed.model, "m-a");
        assert_eq!(parsed.stop_reason, Some(CanonicalStopReason::ToolUse));
        assert_eq!(
            (parsed.usage.input_tokens, parsed.usage.output_tokens),
            (120, 18)
        );
        let texts: Vec<&str> = parsed
            .content
            .iter()
            .filter_map(|part| match part {
                CanonicalContent::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["I will read the file first."]);
        let called = parsed.content.iter().any(|part| {
            matches!(part, CanonicalContent::ToolUse { id, name, input, .. }
                if id == "t1" && name == "read" && input["path"] == "src/lib.rs")
        });
        assert!(called);
    }
}

#[tokio::test]
async fn a_request_that_fails_the_guard_is_refused_and_keeps_the_turn() {
    for protocol in PROTOCOLS {
        let (instance, _) = scripted().await;
        let refused = send(&instance, protocol, &request(protocol, &["read"], &[])).await;
        assert_eq!(refused.status().as_u16(), 400);
        let shown = refused.text().await.unwrap();
        assert!(shown.contains("edit") && shown.contains("read"));
        assert_eq!(instance.unconsumed_turns(), 2);
        let unscripted = instance.unscripted_requests();
        assert_eq!(unscripted.len(), 1);
        assert!(unscripted[0].unscripted);
    }
}

#[tokio::test]
async fn a_request_past_the_last_turn_is_refused_as_unscripted() {
    let instance = instance(ModelScript::empty()).await;
    let protocol = WireProtocol::Anthropic;
    let refused = send(&instance, protocol, &request(protocol, &[], &[])).await;
    assert_eq!(refused.status().as_u16(), 400);
    assert_eq!(instance.unscripted_requests().len(), 1);
}

#[tokio::test]
async fn a_model_call_without_a_credential_takes_no_turn() {
    let (instance, _) = scripted().await;
    let answer = reqwest::Client::new()
        .post(format!(
            "{}{}",
            instance.base_url(),
            ApiPaths::GATEWAY_MESSAGES
        ))
        .json(&request(WireProtocol::Anthropic, &["read", "edit"], &[]))
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status().as_u16(), 401);
    assert_eq!(instance.unconsumed_turns(), 2);
    assert!(instance.unscripted_requests().is_empty());

    let keyed = reqwest::Client::new()
        .post(format!(
            "{}{}",
            instance.base_url(),
            ApiPaths::GATEWAY_MESSAGES
        ))
        .header("x-api-key", instance.pat().as_str())
        .json(&request(WireProtocol::Anthropic, &["read", "edit"], &[]))
        .send()
        .await
        .unwrap();
    assert_eq!(keyed.status().as_u16(), 200);
    assert_eq!(instance.unconsumed_turns(), 1);
}
