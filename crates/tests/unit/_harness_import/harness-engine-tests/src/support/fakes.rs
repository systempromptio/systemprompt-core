use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

use async_trait::async_trait;
use bytes::Bytes;
use harness_engine::{
    EnabledExtensions, Endpoint, GovernError, Governor, McpSpec, ModelCall, ModelSpec, Purpose,
    ResponseAction, ToolCall, Usage, Verdict,
};
use harness_llm::{HttpRequest, HttpResponse, Transport, TransportError};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::sync::Notify;
use url::Url;

use super::model;

#[derive(Default)]
pub(crate) struct TestGovernor {
    pub(crate) deny: Option<String>,
    pub(crate) retry_status: Option<u16>,
    pub(crate) refuse: Option<Purpose>,
    pub(crate) authorized: AtomicU32,
    pub(crate) responses: Mutex<Vec<(u16, u32)>>,
    pub(crate) tool_calls: Mutex<Vec<ToolCall>>,
    pub(crate) enabled: Option<EnabledExtensions>,
    pub(crate) endpoint_headers: Vec<(String, String)>,
}

#[async_trait]
impl Governor for TestGovernor {
    fn models(&self) -> Vec<ModelSpec> {
        vec![model()]
    }

    fn mcp_servers(&self) -> Vec<McpSpec> {
        Vec::new()
    }

    async fn authorize(&self, call: &ModelCall) -> Result<Endpoint, GovernError> {
        if self.refuse == Some(call.purpose) {
            return Err(GovernError::Refused {
                reason: "not for this purpose".to_owned(),
            });
        }
        self.authorized.fetch_add(1, Ordering::SeqCst);
        let mut endpoint = Endpoint::new(Url::parse("http://gateway.test/v1").unwrap());
        endpoint.headers.clone_from(&self.endpoint_headers);
        Ok(endpoint)
    }

    async fn response(&self, _call: &ModelCall, status: u16, attempt: u32) -> ResponseAction {
        self.responses.lock().push((status, attempt));
        if self.retry_status == Some(status) {
            ResponseAction::Retry
        } else {
            ResponseAction::Continue
        }
    }

    async fn step_finished(&self, _call: &ModelCall, _usage: &Usage) {}

    async fn tool_before(&self, call: &ToolCall) -> Verdict {
        self.tool_calls.lock().push(call.clone());
        self.deny
            .as_ref()
            .map_or(Verdict::Allow, |reason| Verdict::Deny {
                reason: reason.clone(),
            })
    }

    fn extensions(&self) -> Option<EnabledExtensions> {
        self.enabled.clone()
    }

    fn generation(&self) -> u64 {
        1
    }
}

#[derive(Default)]
pub(crate) struct Scripted {
    replies: Mutex<VecDeque<(u16, String)>>,
    requests: Mutex<Vec<Value>>,
    request_headers: Mutex<Vec<Vec<(String, String)>>>,
    response_headers: Vec<(String, String)>,
    hold_first: bool,
    pub(crate) first_sent: Notify,
    pub(crate) release: Notify,
}

impl Scripted {
    pub(crate) fn with(replies: Vec<(u16, String)>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            ..Self::default()
        }
    }

    pub(crate) fn held(replies: Vec<(u16, String)>) -> Self {
        Self {
            hold_first: true,
            ..Self::with(replies)
        }
    }

    pub(crate) fn answering_with(mut self, headers: &[(&str, &str)]) -> Self {
        self.response_headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        self
    }

    pub(crate) fn requests(&self) -> Vec<Value> {
        self.requests.lock().clone()
    }

    pub(crate) fn request_headers(&self) -> Vec<Vec<(String, String)>> {
        self.request_headers.lock().clone()
    }
}

#[async_trait]
impl Transport for Scripted {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        self.request_headers.lock().push(request.headers.clone());
        let count = {
            let mut requests = self.requests.lock();
            requests.push(body);
            requests.len()
        };
        if count == 1 && self.hold_first {
            self.first_sent.notify_one();
            self.release.notified().await;
        }
        let next = self.replies.lock().pop_front();
        let (status, text) = next.unwrap_or_else(|| (200, text_reply("(end of script)")));
        let chunks: Vec<Result<Bytes, TransportError>> = vec![Ok(Bytes::from(text))];
        Ok(HttpResponse {
            status,
            headers: self.response_headers.clone(),
            body: Box::pin(futures_util::stream::iter(chunks)),
        })
    }
}

pub(crate) fn text_reply(text: &str) -> String {
    let delta =
        json!({"choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}]});
    let finish = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]});
    let usage = json!({"choices": [], "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}});
    format!("data: {delta}\n\ndata: {finish}\n\ndata: {usage}\n\ndata: [DONE]\n\n")
}

pub(crate) fn tool_reply(id: &str, name: &str, arguments: &Value) -> String {
    let call = json!({"choices": [{"index": 0, "delta": {"tool_calls": [{
        "index": 0, "id": id, "type": "function", "function": {"name": name, "arguments": arguments.to_string()}
    }]}, "finish_reason": null}]});
    let finish = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]});
    format!("data: {call}\n\ndata: {finish}\n\ndata: [DONE]\n\n")
}
