use serde_json::{Value, json};
use systemprompt_scripted_instance::{Aside, Event, ModelScript, Respond, Turn};
use systemprompt_wire::WireProtocol;
use systemprompt_wire::canonical::CanonicalStopReason;

use crate::support::{PROTOCOLS, decode, instance, request, send};

const TITLE_PROMPT: &str = "You are a title generator. Output a short title.";

fn script() -> ModelScript {
    ModelScript::from_turns([Turn {
        respond: vec![Respond::Text("the step".to_owned())],
        ..Turn::default()
    }])
    .with_aside(Aside::title("title generator", "A scripted title"))
}

fn with_system(protocol: WireProtocol, system: &str) -> Value {
    let mut body = request(protocol, &[], &[]);
    match protocol {
        WireProtocol::OpenAiChat => {
            let messages = body["messages"].as_array_mut().unwrap();
            messages.insert(0, json!({ "role": "system", "content": system }));
        },
        _ => body["system"] = json!([{ "type": "text", "text": system }]),
    }
    body
}

#[test]
fn an_aside_reads_from_its_scenario_spelling() {
    let read = ModelScript::from_json(
        r#"{"turns": [],
            "asides": [{"purpose": "title", "system_contains": "title generator",
                        "respond": [{"text": "A scripted title"}]}]}"#,
    )
    .unwrap();
    assert_eq!(
        read.asides,
        [Aside::title("title generator", "A scripted title")]
    );
    assert_eq!(read.asides[0].purpose, Aside::TITLE);
    assert!(
        ModelScript::from_json(r#"{"asides": [{"purpose": "title", "system": "x"}]}"#).is_err()
    );
    let written = serde_json::to_value(ModelScript::empty()).unwrap();
    assert!(written.get("asides").is_none());
}

#[tokio::test]
async fn an_aside_is_answered_beside_the_turns_and_takes_none() {
    for protocol in PROTOCOLS {
        let instance = instance(script()).await;

        let title = send(&instance, protocol, &with_system(protocol, TITLE_PROMPT)).await;
        assert_eq!(title.status().as_u16(), 200);
        let title = decode(protocol, title).await;
        assert_eq!(title.text, "A scripted title");
        assert_eq!(title.stop, Some(CanonicalStopReason::EndTurn));
        assert_eq!(instance.unconsumed_turns(), 1);

        let step = send(
            &instance,
            protocol,
            &with_system(protocol, "You are an agent."),
        )
        .await;
        assert_eq!(decode(protocol, step).await.text, "the step");
        assert_eq!(instance.unconsumed_turns(), 0);

        let late = send(&instance, protocol, &with_system(protocol, TITLE_PROMPT)).await;
        assert_eq!(decode(protocol, late).await.text, "A scripted title");

        assert!(instance.unscripted_requests().is_empty());
        let purposes: Vec<Option<String>> = instance
            .requests()
            .into_iter()
            .map(|record| record.purpose)
            .collect();
        let title = Some(Aside::TITLE.to_owned());
        assert_eq!(purposes, [title.clone(), None, title]);
        assert!(instance.wait_for(Event::ModelCall).await.is_ok());
    }
}

#[tokio::test]
async fn a_buffered_aside_is_one_body() {
    let protocol = WireProtocol::Anthropic;
    let instance = instance(script()).await;
    let mut body = with_system(protocol, TITLE_PROMPT);
    body["stream"] = json!(false);
    body["system"] = json!(TITLE_PROMPT);
    let answer: Value = send(&instance, protocol, &body).await.json().await.unwrap();
    assert_eq!(answer["content"][0]["text"], "A scripted title");
    assert_eq!(instance.unconsumed_turns(), 1);
}

#[tokio::test]
async fn an_aside_without_a_fragment_recognises_nothing() {
    let protocol = WireProtocol::Anthropic;
    let blank = Aside {
        purpose: Aside::TITLE.to_owned(),
        system_contains: String::new(),
        respond: vec![Respond::Text("never sent".to_owned())],
    };
    let instance = instance(ModelScript::empty().with_aside(blank)).await;
    let refused = send(&instance, protocol, &with_system(protocol, TITLE_PROMPT)).await;
    assert_eq!(refused.status().as_u16(), 400);
    let unscripted = instance.unscripted_requests();
    assert_eq!(unscripted.len(), 1);
    assert_eq!(unscripted[0].purpose, None);
}

#[tokio::test]
async fn an_aside_without_a_credential_is_not_answered() {
    let instance = instance(script()).await;
    let answer = reqwest::Client::new()
        .post(format!(
            "{}{}",
            instance.base_url(),
            crate::support::path(WireProtocol::Anthropic)
        ))
        .json(&with_system(WireProtocol::Anthropic, TITLE_PROMPT))
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status().as_u16(), 401);
    assert_eq!(instance.requests()[0].purpose, None);
}
