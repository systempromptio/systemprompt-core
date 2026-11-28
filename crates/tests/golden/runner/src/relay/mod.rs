//! A loopback relay in front of the scripted instance. It answers the
//! session-title call itself, re-signs the manifest with the model families
//! the harness reads ([`manifest`]), and forwards everything else unchanged,
//! streaming each answer back as it arrives.
//!
//! The harness names a session with a model call that runs beside the first
//! step, so it has no fixed place among the scripted turns, and the scripted
//! instance has no way to script it (S2-A1 hand-off, item 1). The corpus's rule
//! decides what a title call is: a model request that advertises no tool and
//! carries one user message. It is answered with `[title_call] text` in the
//! protocol and framing the request asked for.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse as _, Response};
use serde_json::Value;
use systemprompt_models::ApiPaths;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

mod manifest;
mod title;

const BODY_LIMIT: usize = 64 << 20;

#[derive(Debug)]
struct Shared {
    upstream: String,
    family: &'static str,
    key: manifest::RelayKey,
    title: String,
    client: reqwest::Client,
    titles: AtomicUsize,
}

#[derive(Debug)]
pub struct Relay {
    url: String,
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Relay {
    /// `family` is the client family the instance's one model is served to:
    /// `claude` for the messages protocol, `openai` for chat.
    pub async fn start(upstream: &str, title: &str, family: &'static str) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let shared = Arc::new(Shared {
            upstream: upstream.trim_end_matches('/').to_owned(),
            family,
            key: manifest::RelayKey::generate(),
            title: title.to_owned(),
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            titles: AtomicUsize::new(0),
        });
        let app = axum::Router::new()
            .fallback(|state: State<Arc<Shared>>, request: Request| async move {
                relay(&state.0, request).await.unwrap_or_else(|error| {
                    (StatusCode::BAD_GATEWAY, error.to_string()).into_response()
                })
            })
            .with_state(Arc::clone(&shared));
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, app).await {
                eprintln!("golden relay stopped: {error}");
            }
        });
        Ok(Self { url, shared, task })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// How many title calls the relay answered.
    pub fn title_calls(&self) -> usize {
        self.shared.titles.load(Ordering::SeqCst)
    }
}

fn is_model_route(path: &str) -> bool {
    path == ApiPaths::GATEWAY_MESSAGES || path == ApiPaths::GATEWAY_CHAT_COMPLETIONS
}

/// The corpus's rule: no tool advertised and exactly one user message.
pub fn is_title_call(body: &Value) -> bool {
    let tools = body
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| !tools.is_empty());
    let messages: Vec<&Value> = body
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|message| message.get("role").and_then(Value::as_str) != Some("system"))
        .collect();
    !tools && messages.len() == 1 && messages[0].get("role").and_then(Value::as_str) == Some("user")
}

async fn relay(shared: &Shared, request: Request) -> anyhow::Result<Response> {
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, BODY_LIMIT).await?;
    if is_model_route(parts.uri.path())
        && let Ok(json) = serde_json::from_slice::<Value>(&bytes)
        && is_title_call(&json)
    {
        shared.titles.fetch_add(1, Ordering::SeqCst);
        return title::answer(parts.uri.path(), &json, &shared.title);
    }
    if parts.uri.path() == ApiPaths::BRIDGE_PUBKEY {
        return Ok(axum::Json(shared.key.pubkey()).into_response());
    }
    let target = parts.uri.path_and_query().map_or("/", |path| path.as_str());
    let mut outgoing = shared
        .client
        .request(parts.method.clone(), format!("{}{target}", shared.upstream))
        .body(bytes);
    for (name, value) in &parts.headers {
        if name != header::HOST && name != header::CONTENT_LENGTH {
            outgoing = outgoing.header(name, value);
        }
    }
    let answer = outgoing.send().await?;
    let mut response = Response::builder().status(answer.status());
    for (name, value) in answer.headers() {
        if name != header::TRANSFER_ENCODING
            && name != header::CONTENT_LENGTH
            && name != header::CONNECTION
        {
            response = response.header(name, value);
        }
    }
    let rewrite = answer.status() == StatusCode::OK
        && (parts.uri.path() == ApiPaths::BRIDGE_MANIFEST
            || parts.uri.path() == ApiPaths::BRIDGE_PROFILE);
    if rewrite {
        let bytes = answer.bytes().await?;
        let rewritten = if parts.uri.path() == ApiPaths::BRIDGE_MANIFEST {
            shared.key.resign(&bytes)?
        } else {
            manifest::with_provider(&bytes, shared.family)?
        };
        return Ok(response.body(Body::from(rewritten))?);
    }
    Ok(response.body(Body::from_stream(answer.bytes_stream()))?)
}
