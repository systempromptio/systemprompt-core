use std::time::{Duration, Instant};

use systemprompt_scripted_instance::{ModelScript, Respond, Turn, TurnFault};

use crate::support::{PROTOCOLS, decode, instance, request, send};

fn faulted(fault: TurnFault) -> ModelScript {
    ModelScript::from_turns([Turn {
        respond: vec![
            Respond::Text("one".to_owned()),
            Respond::Text("two".to_owned()),
        ],
        fault: Some(fault),
        ..Turn::default()
    }])
}

#[test]
fn every_fault_reads_from_its_scenario_spelling() {
    let script = ModelScript::from_json(
        r#"{"turns": [{"fault": {"status": 429}}, {"fault": {"close_after_events": 2}},
                      {"fault": {"stall_ms": 5000}}, {"fault": "malformed_event"}]}"#,
    )
    .unwrap();
    let faults: Vec<_> = script
        .turns
        .iter()
        .map(|turn| turn.fault.unwrap())
        .collect();
    assert_eq!(
        faults,
        [
            TurnFault::Status(429),
            TurnFault::CloseAfterEvents(2),
            TurnFault::StallMs(5000),
            TurnFault::MalformedEvent,
        ]
    );
    assert!(ModelScript::from_json(r#"{"turns": [{"respnd": []}]}"#).is_err());
}

#[tokio::test]
async fn a_status_fault_answers_with_the_status_and_takes_the_turn() {
    for protocol in PROTOCOLS {
        for status in [401, 429, 500, 529] {
            let instance = instance(faulted(TurnFault::Status(status))).await;
            let answer = send(&instance, protocol, &request(protocol, &[], &[])).await;
            assert_eq!(answer.status().as_u16(), status);
            assert_eq!(answer.headers()["retry-after-ms"], "1");
            let body: serde_json::Value = answer.json().await.unwrap();
            assert!(body["error"]["message"].is_string());
            assert_eq!(instance.unconsumed_turns(), 0);
            assert!(instance.unscripted_requests().is_empty());
        }
    }
}

#[tokio::test]
async fn a_closed_stream_ends_without_a_stop() {
    for protocol in PROTOCOLS {
        let instance = instance(faulted(TurnFault::CloseAfterEvents(2))).await;
        let answer = send(&instance, protocol, &request(protocol, &[], &[])).await;
        assert_eq!(answer.status().as_u16(), 200);
        let decoded = decode(protocol, answer).await;
        assert_eq!(decoded.stop, None);
        assert_eq!(decoded.failed, 1);
        assert!(!decoded.text.contains("two"));
    }
}

#[tokio::test]
async fn a_malformed_event_is_dropped_by_the_decoder() {
    for protocol in PROTOCOLS {
        let instance = instance(faulted(TurnFault::MalformedEvent)).await;
        let answer = send(&instance, protocol, &request(protocol, &[], &[])).await;
        let decoded = decode(protocol, answer).await;
        assert_eq!(decoded.dropped, 1);
        assert!(decoded.text.is_empty());
    }
}

#[tokio::test]
async fn a_stalled_stream_sends_its_answer_then_holds_the_end() {
    for protocol in PROTOCOLS {
        let instance = instance(faulted(TurnFault::StallMs(300))).await;
        let began = Instant::now();
        let mut answer = send(&instance, protocol, &request(protocol, &[], &[])).await;
        let mut early = String::new();
        while !early.contains("two") {
            let chunk = answer.chunk().await.unwrap().unwrap();
            early.push_str(std::str::from_utf8(&chunk).unwrap());
        }
        assert!(began.elapsed() < Duration::from_millis(300));
        assert!(!early.contains("message_stop") && !early.contains("[DONE]"));
        let mut rest = String::new();
        while let Some(chunk) = answer.chunk().await.unwrap() {
            rest.push_str(std::str::from_utf8(&chunk).unwrap());
        }
        assert!(began.elapsed() >= Duration::from_millis(300));
        assert!(rest.contains("message_stop") || rest.contains("[DONE]"));
    }
}
