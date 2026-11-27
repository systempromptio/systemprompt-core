//! The three ways an extension takes part in governed calls without being the
//! Governor: it observes, it reviews a tool call the Governor allowed, and it
//! adds request headers to a model call. Each is held as `dyn`, hence
//! `#[async_trait]`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use harness_ids::ExtensionId;
use harness_llm::{Endpoint, ResponseHead};

use crate::governor::{ModelCall, SessionEvent, ToolCall, ToolOutcome, UserPrompt};

/// Hears what the Governor hears, after it, and changes nothing.
///
/// Held as `dyn`, which is why the trait uses `async_trait`. Every method has
/// an empty default, so an observer implements only what it reads.
/// `user_prompt` comes once per new user message before its first step,
/// `tool_after` once a tool call the Governor reviewed ended, `session_event`
/// on each session lifecycle moment, and `response_head` once a model response
/// opened with a success status and the Governor let it through.
#[async_trait]
pub trait GovernorObserver: Send + Sync {
    async fn user_prompt(&self, _prompt: &UserPrompt) {}

    async fn tool_after(&self, _call: &ToolCall, _outcome: &ToolOutcome) {}

    fn session_event(&self, _event: &SessionEvent) {}

    async fn response_head(&self, _call: &ModelCall, _head: &ResponseHead) {}
}

/// An extension's second look at a tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Review {
    Pass,
    Deny { reason: String },
}

/// Reviews a tool call after the Governor allowed it.
///
/// Held as `dyn`, which is why the trait uses `async_trait`. It runs only
/// once the Governor's verdict was `Allow` and can only take that away: a
/// `Review::Deny` stops the tool and its reason goes back to the model, a
/// `Review::Pass` leaves the Governor's verdict standing.
#[async_trait]
pub trait ToolReviewer: Send + Sync {
    async fn review(&self, call: &ToolCall) -> Review;
}

/// Adds request headers to a model call the Governor authorised.
///
/// Held as `dyn`, which is why the trait uses `async_trait`. The headers are
/// merged with [`add_headers`]: a header whose name the endpoint already
/// carries, from the Governor or an earlier augmenter, is dropped, so an
/// augmenter never overrides a governed header.
#[async_trait]
pub trait CallAugmenter: Send + Sync {
    async fn headers(&self, call: &ModelCall) -> Vec<(String, String)>;
}

pub fn add_headers(
    endpoint: &mut Endpoint,
    extension: &ExtensionId,
    headers: Vec<(String, String)>,
) {
    for (name, value) in headers {
        let taken = endpoint
            .headers
            .iter()
            .any(|(present, _)| present.eq_ignore_ascii_case(&name));
        if taken {
            tracing::warn!(
                extension = %extension,
                header = %name,
                "an extension header names a header the call already carries; dropped"
            );
            continue;
        }
        endpoint.headers.push((name, value));
    }
}
