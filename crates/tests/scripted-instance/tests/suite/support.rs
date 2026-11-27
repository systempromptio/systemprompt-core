use futures_util::StreamExt;
use serde_json::{Value, json};
use systemprompt_device::client::{HttpClient, JwtSession, SpClient};
use systemprompt_device::protocol::Pat;
use systemprompt_scripted_instance::{ModelFixture, ModelScript, Respond, ScriptedInstance, Turn};
use systemprompt_wire::canonical::{CanonicalEvent, CanonicalStopReason, ContentBlockKind};
use systemprompt_wire::{WireProtocol, anthropic, openai_chat};

pub(crate) const USER_AGENT: &str = "scripted-instance-tests";
pub(crate) const PROTOCOLS: [WireProtocol; 2] = [WireProtocol::Anthropic, WireProtocol::OpenAiChat];

pub(crate) async fn instance(script: ModelScript) -> ScriptedInstance {
    ScriptedInstance::builder()
        .models([ModelFixture::messages("m-a"), ModelFixture::chat("m-b")])
        .script(script)
        .start()
        .await
        .unwrap()
}

pub(crate) fn client(instance: &ScriptedInstance) -> SpClient {
    SpClient::new(instance.base_url(), HttpClient::new(USER_AGENT).unwrap()).unwrap()
}

pub(crate) async fn signed_in(instance: &ScriptedInstance) -> (SpClient, JwtSession) {
    let client = client(instance);
    let pat = Pat::new(instance.pat().as_str()).unwrap();
    let session = client.mint_session(&pat, None).await.unwrap();
    (client, session)
}

pub(crate) fn path(protocol: WireProtocol) -> &'static str {
    match protocol {
        WireProtocol::OpenAiChat => systemprompt_models::ApiPaths::GATEWAY_CHAT_COMPLETIONS,
        _ => systemprompt_models::ApiPaths::GATEWAY_MESSAGES,
    }
}

pub(crate) fn request(protocol: WireProtocol, tools: &[&str], results: &[&str]) -> Value {
    let mut messages = vec![json!({ "role": "user", "content": "go" })];
    let advertised: Vec<Value> = match protocol {
        WireProtocol::OpenAiChat => {
            messages.extend(
                results
                    .iter()
                    .map(|id| json!({ "role": "tool", "tool_call_id": id, "content": "ok" })),
            );
            tools
                .iter()
                .map(|name| json!({ "type": "function", "function": { "name": name } }))
                .collect()
        },
        _ => {
            if !results.is_empty() {
                let blocks: Vec<Value> = results
                    .iter()
                    .map(|id| json!({ "type": "tool_result", "tool_use_id": id, "content": "ok" }))
                    .collect();
                messages.push(json!({ "role": "user", "content": blocks }));
            }
            tools
                .iter()
                .map(|name| json!({ "name": name, "input_schema": { "type": "object" } }))
                .collect()
        },
    };
    json!({
        "model": "m-a",
        "max_tokens": 64,
        "stream": true,
        "messages": messages,
        "tools": advertised,
    })
}

pub(crate) async fn send(
    instance: &ScriptedInstance,
    protocol: WireProtocol,
    body: &Value,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("{}{}", instance.base_url(), path(protocol)))
        .bearer_auth(instance.pat().as_str())
        .json(body)
        .send()
        .await
        .unwrap()
}

#[derive(Debug, Default)]
pub(crate) struct Decoded {
    pub(crate) text: String,
    pub(crate) tools: Vec<DecodedTool>,
    pub(crate) stop: Option<CanonicalStopReason>,
    pub(crate) input: u32,
    pub(crate) output: u32,
    pub(crate) dropped: usize,
    pub(crate) failed: usize,
}

#[derive(Debug, Default)]
pub(crate) struct DecodedTool {
    index: u32,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) chunks: Vec<String>,
}

impl Decoded {
    pub(crate) fn assert_is(&self, turn: &Turn) {
        let text: String = turn
            .respond
            .iter()
            .filter_map(|part| match part {
                Respond::Text(text) => Some(text.as_str()),
                Respond::ToolCall(_) => None,
            })
            .collect();
        assert_eq!(self.text, text);
        let calls: Vec<_> = turn
            .respond
            .iter()
            .filter_map(|part| match part {
                Respond::ToolCall(call) => Some(call),
                Respond::Text(_) => None,
            })
            .collect();
        assert_eq!(self.tools.len(), calls.len());
        for (decoded, scripted) in self.tools.iter().zip(calls) {
            assert_eq!(decoded.id, scripted.id);
            assert_eq!(decoded.name, scripted.name);
            let input: Value = serde_json::from_str(&decoded.chunks.concat()).unwrap();
            assert_eq!(input, scripted.input);
        }
        let usage = turn.usage.unwrap_or_default();
        assert_eq!((self.input, self.output), (usage.input, usage.output));
        assert_eq!((self.dropped, self.failed), (0, 0));
    }
}

pub(crate) async fn decode(protocol: WireProtocol, response: reqwest::Response) -> Decoded {
    let bytes = response.bytes_stream();
    let mut events = match protocol {
        WireProtocol::OpenAiChat => openai_chat::sse_to_canonical_events(bytes, "none".to_owned()),
        _ => anthropic::sse_to_canonical_events(bytes),
    };
    let mut decoded = Decoded::default();
    while let Some(event) = events.next().await {
        let Ok(event) = event else {
            decoded.failed += 1;
            continue;
        };
        match event {
            CanonicalEvent::MessageStart { usage, .. } => decoded.input = usage.input_tokens,
            CanonicalEvent::ContentBlockStart {
                index,
                block: ContentBlockKind::ToolUse { id, name, .. },
            } => decoded.tools.push(DecodedTool {
                index,
                id,
                name,
                chunks: Vec::new(),
            }),
            CanonicalEvent::TextDelta { text, .. } => decoded.text.push_str(&text),
            CanonicalEvent::ToolUseDelta {
                index,
                partial_json,
            } => {
                let tool = decoded.tools.iter_mut().find(|tool| tool.index == index);
                tool.unwrap().chunks.push(partial_json);
            },
            CanonicalEvent::UsageDelta(update) => {
                decoded.input = update.input_tokens.unwrap_or(decoded.input);
                decoded.output = update.output_tokens.unwrap_or(decoded.output);
            },
            CanonicalEvent::MessageStop {
                stop_reason: Some(reason),
                ..
            } => decoded.stop = Some(reason),
            CanonicalEvent::FrameDropped { .. } => decoded.dropped += 1,
            CanonicalEvent::Error(_) => decoded.failed += 1,
            _ => {},
        }
    }
    decoded
}
