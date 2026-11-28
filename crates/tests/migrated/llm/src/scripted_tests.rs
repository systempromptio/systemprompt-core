//! Model calls driven end to end through the scripted instance: the engine's
//! client sends real HTTP to the scripted model on loopback and decodes what
//! it streams back, on both protocols.

use std::time::Duration;

use futures_util::StreamExt;
use serde_json::{Value, json};
use systemprompt_engine::llm::retry::{Retry, classify};
use systemprompt_engine::llm::{
    Attachment, Content, Endpoint, Event, FinishReason, HttpTransport, LlmError, Message,
    ModelResponse, Protocol, Request, Timeouts, TransportError, stream,
};
use systemprompt_scripted_instance::{
    Expect, ModelFixture, ModelScript, RecordedBody, Respond, Route, ScriptedInstance, ToolCall,
    Turn, TurnFault, Usage,
};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::support::{all_supports, finish, model, texts, tool_ends};

const MODEL: &str = "scripted-model";
const PROTOCOLS: [Protocol; 2] = [Protocol::Messages, Protocol::ChatCompletions];

struct Scripted {
    instance: ScriptedInstance,
    endpoint: Endpoint,
    protocol: Protocol,
}

impl Scripted {
    async fn start(protocol: Protocol, turns: Vec<Turn>) -> Self {
        let fixture = match protocol {
            Protocol::Messages => ModelFixture::messages(MODEL),
            Protocol::ChatCompletions => ModelFixture::chat(MODEL),
        };
        let instance = ScriptedInstance::builder()
            .models([fixture])
            .script(ModelScript::from_turns(turns))
            .start()
            .await
            .unwrap();
        let base =
            Url::parse(&format!("{}/v1", instance.base_url().trim_end_matches('/'))).unwrap();
        let mut endpoint = Endpoint::new(base);
        endpoint.headers.push((
            "authorization".into(),
            format!("Bearer {}", instance.pat().as_str()),
        ));
        Self {
            instance,
            endpoint,
            protocol,
        }
    }

    fn request(&self, messages: Vec<Message>) -> Request {
        let mut spec = model(self.protocol, all_supports());
        spec.id = MODEL.into();
        let mut request = Request::new(spec);
        request.messages = messages;
        request
    }

    async fn open(&self, request: &Request, timeouts: Timeouts) -> Result<ModelResponse, LlmError> {
        let transport = HttpTransport::with_timeouts(timeouts).unwrap();
        stream(
            &transport,
            &self.endpoint,
            request,
            CancellationToken::new(),
        )
        .await
    }

    async fn run(&self, request: &Request) -> Vec<Result<Event, LlmError>> {
        let response = self.open(request, Timeouts::default()).await.unwrap();
        response.events.collect().await
    }

    async fn run_ok(&self, request: &Request) -> Vec<Event> {
        self.run(request)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    fn model_bodies(&self) -> Vec<Value> {
        self.instance
            .requests()
            .into_iter()
            .filter(|r| matches!(r.route, Route::Messages | Route::ChatCompletions))
            .filter_map(|r| match r.body {
                RecordedBody::Json(value) => Some(value),
                _ => None,
            })
            .collect()
    }

    async fn finish(self) {
        assert!(
            self.instance.unscripted_requests().is_empty(),
            "a request was refused by the script"
        );
        self.instance.shutdown().await.unwrap();
    }
}

fn text_turn(text: &str) -> Turn {
    Turn {
        respond: vec![Respond::Text(text.into())],
        usage: Some(Usage {
            input: 120,
            output: 18,
        }),
        ..Turn::default()
    }
}

fn tool_turn(id: &str, input: Value, chunks: &[&str]) -> Turn {
    Turn {
        respond: vec![Respond::ToolCall(ToolCall {
            id: id.into(),
            name: "read".into(),
            input,
            argument_chunks: chunks.iter().map(|c| (*c).to_owned()).collect(),
        })],
        usage: Some(Usage {
            input: 50,
            output: 9,
        }),
        ..Turn::default()
    }
}

fn fault(fault: TurnFault, respond: &str) -> Turn {
    Turn {
        respond: vec![Respond::Text(respond.into())],
        fault: Some(fault),
        ..Turn::default()
    }
}

fn tool_history(id: &str, attachments: Vec<Attachment>) -> Vec<Message> {
    vec![
        Message::user_text("read the file"),
        Message::assistant(vec![Content::ToolCall {
            id: id.into(),
            name: "read".into(),
            input: json!({"path": "a.txt"}),
        }]),
        Message::user(vec![Content::ToolResult {
            id: id.into(),
            output: "file contents".into(),
            is_error: false,
            attachments,
        }]),
    ]
}

fn usage(events: &[Event]) -> Option<systemprompt_engine::llm::Usage> {
    events.iter().find_map(|e| match e {
        Event::Usage(u) => Some(*u),
        _ => None,
    })
}

#[tokio::test]
async fn a_plain_text_answer_decodes_on_both_protocols() {
    for protocol in PROTOCOLS {
        let scripted = Scripted::start(protocol, vec![text_turn("Hello there.")]).await;
        let events = scripted
            .run_ok(&scripted.request(vec![Message::user_text("hi")]))
            .await;
        assert_eq!(texts(&events), "Hello there.", "{protocol:?}");
        assert_eq!(finish(&events), Some(&FinishReason::Stop), "{protocol:?}");
        scripted.finish().await;
    }
}

#[tokio::test]
async fn a_single_tool_call_decodes_with_its_identity_and_input_on_both_protocols() {
    for protocol in PROTOCOLS {
        let scripted = Scripted::start(
            protocol,
            vec![tool_turn(
                "t1",
                json!({"path": "a.txt"}),
                &["{\"pa", "th\": \"a.txt\"}"],
            )],
        )
        .await;
        let events = scripted
            .run_ok(&scripted.request(vec![Message::user_text("read a.txt")]))
            .await;
        assert_eq!(
            tool_ends(&events),
            vec![("t1".into(), "read".into(), json!({"path": "a.txt"}), None)],
            "{protocol:?}"
        );
        assert_eq!(
            finish(&events),
            Some(&FinishReason::ToolCalls),
            "{protocol:?}"
        );
        scripted.finish().await;
    }
}

#[tokio::test]
async fn a_multi_step_tool_loop_sends_the_result_back_and_ends_with_text_on_both_protocols() {
    for protocol in PROTOCOLS {
        let mut answer = text_turn("The file says hello.");
        answer.expect = Expect {
            tools_include: vec![],
            tool_results: vec!["t1".into()],
        };
        let scripted = Scripted::start(
            protocol,
            vec![tool_turn("t1", json!({"path": "a.txt"}), &[]), answer],
        )
        .await;
        let first = scripted
            .run_ok(&scripted.request(vec![Message::user_text("read the file")]))
            .await;
        assert_eq!(tool_ends(&first).len(), 1);
        let second = scripted
            .run_ok(&scripted.request(tool_history("t1", vec![])))
            .await;
        assert_eq!(texts(&second), "The file says hello.", "{protocol:?}");
        assert_eq!(scripted.instance.unconsumed_turns(), 0);
        scripted.finish().await;
    }
}

#[tokio::test]
async fn an_image_returned_by_a_tool_reaches_the_model_and_the_answer_decodes_on_both_protocols() {
    for protocol in PROTOCOLS {
        let mut answer = text_turn("A red square.");
        answer.expect = Expect {
            tools_include: vec![],
            tool_results: vec!["t1".into()],
        };
        let scripted = Scripted::start(protocol, vec![answer]).await;
        let image = Attachment {
            mime: "image/png".into(),
            data: "iVBORw0KGgo=".into(),
            name: None,
        };
        let events = scripted
            .run_ok(&scripted.request(tool_history("t1", vec![image])))
            .await;
        assert_eq!(texts(&events), "A red square.");
        let sent = scripted.model_bodies().remove(0).to_string();
        assert!(sent.contains("iVBORw0KGgo="), "{protocol:?}: {sent}");
        scripted.finish().await;
    }
}

#[tokio::test]
async fn a_pdf_from_the_user_and_a_pdf_from_a_tool_reach_the_model_as_documents_on_both_protocols()
{
    for protocol in PROTOCOLS {
        let mut answer = text_turn("Read both.");
        answer.expect = Expect {
            tools_include: vec![],
            tool_results: vec!["t1".into()],
        };
        let scripted = Scripted::start(protocol, vec![answer]).await;
        let pdf = Attachment {
            mime: "application/pdf".into(),
            data: "JVBERi0xLjQ=".into(),
            name: Some("tool.pdf".into()),
        };
        let mut messages = vec![Message::user(vec![
            Content::text("compare"),
            Content::Document {
                mime: "application/pdf".into(),
                data: "JVBERi0xLjc=".into(),
                name: Some("user.pdf".into()),
            },
        ])];
        messages.extend(tool_history("t1", vec![pdf]).into_iter().skip(1));
        scripted.run_ok(&scripted.request(messages)).await;
        let sent = scripted.model_bodies().remove(0);
        let text = sent.to_string();
        let (user_pdf, tool_pdf) = match protocol {
            Protocol::Messages => (
                sent["messages"][0]["content"][1]["type"] == "document",
                text.contains("\"type\":\"document\"") && text.contains("JVBERi0xLjQ="),
            ),
            Protocol::ChatCompletions => (
                sent["messages"][0]["content"][1]["type"] == "file",
                text.contains("data:application/pdf;base64,JVBERi0xLjQ="),
            ),
        };
        assert!(user_pdf && tool_pdf, "{protocol:?}: {text}");
        scripted.finish().await;
    }
}

#[tokio::test]
async fn malformed_streamed_tool_input_is_flagged_and_not_executable() {
    for protocol in PROTOCOLS {
        let scripted = Scripted::start(
            protocol,
            vec![tool_turn("t1", json!({"path": "a"}), &["{\"path\": "])],
        )
        .await;
        let events = scripted
            .run_ok(&scripted.request(vec![Message::user_text("go")]))
            .await;
        let (_, _, input, malformed) = tool_ends(&events).remove(0);
        assert_eq!(input, json!({}), "{protocol:?}");
        assert_eq!(malformed.as_deref(), Some("{\"path\": "), "{protocol:?}");
        scripted.finish().await;
    }
}

#[tokio::test]
async fn a_rate_limit_status_is_retryable_after_the_servers_wait() {
    for protocol in PROTOCOLS {
        let scripted = Scripted::start(protocol, vec![fault(TurnFault::Status(429), "")]).await;
        let error = scripted
            .open(
                &scripted.request(vec![Message::user_text("hi")]),
                Timeouts::default(),
            )
            .await
            .err()
            .unwrap();
        assert!(
            matches!(error, LlmError::Status { status: 429, .. }),
            "{protocol:?}: {error:?}"
        );
        assert_eq!(classify(&error), Retry::After(Duration::from_millis(1)));
        scripted.finish().await;
    }
}

#[tokio::test]
async fn a_request_the_endpoint_rejects_is_an_invalid_request_carrying_its_message() {
    for protocol in PROTOCOLS {
        let scripted = Scripted::start(protocol, vec![fault(TurnFault::Status(400), "")]).await;
        let error = scripted
            .open(
                &scripted.request(vec![Message::user_text("hi")]),
                Timeouts::default(),
            )
            .await
            .err()
            .unwrap();
        match &error {
            LlmError::Status {
                status: 400,
                body_excerpt,
                ..
            } => assert!(!body_excerpt.is_empty(), "{protocol:?}"),
            other => panic!("{protocol:?}: expected a 400, got {other:?}"),
        }
        assert_eq!(classify(&error), Retry::No);
        scripted.finish().await;
    }
}

#[tokio::test]
async fn a_dropped_connection_keeps_the_output_before_it_and_fails_as_a_retryable_read_error() {
    for protocol in PROTOCOLS {
        let mut turn = text_turn("partial answer");
        turn.fault = Some(TurnFault::CloseAfterEvents(3));
        let scripted = Scripted::start(protocol, vec![turn]).await;
        let response = scripted
            .open(
                &scripted.request(vec![Message::user_text("hi")]),
                Timeouts::default(),
            )
            .await
            .unwrap();
        assert_eq!(response.head.status, 200);
        let items: Vec<_> = response.events.collect().await;
        let ok: Vec<Event> = items
            .iter()
            .filter_map(|i| i.as_ref().ok().cloned())
            .collect();
        assert_eq!(texts(&ok), "partial answer", "{protocol:?}");
        assert!(finish(&ok).is_none(), "{protocol:?}");
        let error = items
            .iter()
            .find_map(|i| i.as_ref().err())
            .expect("the cut stream did not fail");
        assert!(
            matches!(error, LlmError::Transport(_) | LlmError::Truncated),
            "{protocol:?}: {error:?}"
        );
        if matches!(error, LlmError::Transport(_)) {
            assert!(std::error::Error::source(error).is_some());
        }
        assert_eq!(classify(error), Retry::Backoff);
        scripted.finish().await;
    }
}

#[tokio::test]
async fn a_body_that_stalls_past_the_wait_for_data_fails_as_a_read_timeout() {
    for protocol in PROTOCOLS {
        let scripted = Scripted::start(
            protocol,
            vec![fault(TurnFault::StallMs(5_000), "before the stall")],
        )
        .await;
        let timeouts = Timeouts {
            connect: Duration::from_secs(5),
            idle: Duration::from_millis(400),
        };
        let response = scripted
            .open(&scripted.request(vec![Message::user_text("hi")]), timeouts)
            .await
            .unwrap();
        let items: Vec<_> = response.events.collect().await;
        let error = items
            .iter()
            .find_map(|i| i.as_ref().err())
            .expect("the stalled stream did not fail");
        assert!(
            matches!(error, LlmError::Transport(TransportError::Timeout)),
            "{protocol:?}: {error:?}"
        );
        assert_eq!(classify(error), Retry::Backoff);
        scripted.finish().await;
    }
}

#[tokio::test]
async fn an_undecodable_event_fails_the_stream_and_the_response_head_is_kept() {
    for protocol in PROTOCOLS {
        let scripted = Scripted::start(protocol, vec![fault(TurnFault::MalformedEvent, "x")]).await;
        let response = scripted
            .open(
                &scripted.request(vec![Message::user_text("hi")]),
                Timeouts::default(),
            )
            .await
            .unwrap();
        assert_eq!(response.head.status, 200);
        assert!(response.head.header("content-type").is_some());
        let items: Vec<_> = response.events.collect().await;
        assert!(
            items.iter().any(|i| matches!(i, Err(LlmError::Decode(_)))),
            "{protocol:?}"
        );
        scripted.finish().await;
    }
}

#[tokio::test]
async fn usage_reported_by_the_stream_is_carried_on_the_answer() {
    for protocol in PROTOCOLS {
        let scripted = Scripted::start(protocol, vec![text_turn("ok")]).await;
        let events = scripted
            .run_ok(&scripted.request(vec![Message::user_text("hi")]))
            .await;
        let usage = usage(&events).expect("no usage");
        assert_eq!((usage.input, usage.output), (120, 18), "{protocol:?}");
        scripted.finish().await;
    }
}
