//! Opening a model call under governance: authorise, send, let the Governor
//! judge the response head (one Retry per step), let observers hear the head
//! of a response that opened, and back off on transient failures with a
//! visible Retry status.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::llm::retry::{self, Retry};
use crate::llm::{Event, EventStream, FinishReason, LlmError, ModelResponse, Request, Usage};
use futures_util::StreamExt;
use systemprompt_traits::CancellationToken;

use crate::run::error::EngineError;
use crate::run::events::RunStatus;
use crate::run::governor::{ModelCall, Purpose, ResponseAction};
use crate::run::inner::Inner;

pub(crate) const MAX_ATTEMPTS: u32 = 6;

#[derive(Debug, Default)]
pub(crate) struct Attempts {
    pub(crate) count: u32,
    governor_retry: bool,
}

pub(crate) async fn open(
    inner: &Inner,
    call: &ModelCall,
    request: &Request,
    cancel: &CancellationToken,
    attempts: &mut Attempts,
) -> Result<EventStream, EngineError> {
    loop {
        attempts.count += 1;
        let endpoint = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(EngineError::Aborted),
            authorized = inner.governed.authorize(call) => {
                authorized.map_err(EngineError::Governance)?
            },
        };
        let transport = inner.transport.as_ref();
        let opened = crate::llm::stream(transport, &endpoint, request, cancel.clone()).await;
        if let Some(status) = head_status(&opened) {
            match inner.governed.response(call, status, attempts.count).await {
                ResponseAction::Retry if !attempts.governor_retry => {
                    attempts.governor_retry = true;
                    tracing::info!(
                        session = %call.session_id,
                        status,
                        attempt = attempts.count,
                        "governor asked to resend the call"
                    );
                    continue;
                },
                ResponseAction::Fail(reason) => {
                    tracing::warn!(session = %call.session_id, status, "governor failed the call");
                    return Err(EngineError::Refused { reason });
                },
                ResponseAction::Retry | ResponseAction::Continue => {},
            }
        }
        match opened {
            Ok(response) => {
                inner.governed.response_head(call, &response.head).await;
                return Ok(response.events);
            },
            Err(error) => {
                if !retryable(&error, attempts) {
                    return Err(EngineError::from_llm(error));
                }
                backoff(inner, call, attempts, &error, cancel).await?;
            },
        }
    }
}

const fn head_status(opened: &Result<ModelResponse, LlmError>) -> Option<u16> {
    match opened {
        Ok(response) => Some(response.head.status),
        Err(
            LlmError::Status { status, .. }
            | LlmError::ContextOverflow {
                status: Some(status),
                ..
            },
        ) => Some(*status),
        Err(_) => None,
    }
}

pub(crate) fn retryable(error: &LlmError, attempts: &Attempts) -> bool {
    attempts.count < MAX_ATTEMPTS && retry::classify(error) != Retry::No
}

pub(crate) async fn backoff(
    inner: &Inner,
    call: &ModelCall,
    attempts: &Attempts,
    error: &LlmError,
    cancel: &CancellationToken,
) -> Result<(), EngineError> {
    let wait = match retry::classify(error) {
        Retry::After(wait) => wait,
        Retry::Backoff | Retry::No => retry::delay(attempts.count, None),
    };
    let visible = matches!(call.purpose, Purpose::Step | Purpose::Compaction);
    if visible {
        let status = RunStatus::Retry {
            attempt: attempts.count,
            message: error.to_string(),
            next_ms: u64::try_from(wait.as_millis()).unwrap_or(u64::MAX),
        };
        inner
            .runs
            .set_status(&call.session_id, status, &inner.events);
    }
    tracing::info!(
        session = %call.session_id,
        attempt = attempts.count,
        wait_ms = wait.as_millis(),
        error = %error,
        "retrying the model call"
    );
    tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(EngineError::Aborted),
        () = tokio::time::sleep(wait) => {}
    }
    if visible {
        inner
            .runs
            .set_status(&call.session_id, RunStatus::Busy, &inner.events);
    }
    Ok(())
}

#[derive(Debug, Default)]
pub(crate) struct Collected {
    pub(crate) text: String,
    pub(crate) usage: Usage,
    pub(crate) finish: Option<FinishReason>,
}

impl Collected {
    pub(crate) const fn cut_off(&self) -> bool {
        matches!(
            self.finish,
            Some(FinishReason::Length | FinishReason::ContentFilter)
        )
    }
}

pub(crate) async fn collect_text(
    inner: &Inner,
    call: &ModelCall,
    request: &Request,
    cancel: &CancellationToken,
) -> Result<Collected, EngineError> {
    let mut attempts = Attempts::default();
    loop {
        let mut events = open(inner, call, request, cancel, &mut attempts).await?;
        let mut collected = Collected::default();
        let mut failure = None;
        while let Some(item) = events.next().await {
            match item {
                Ok(Event::TextDelta(text)) => collected.text.push_str(&text),
                Ok(Event::Usage(usage)) => collected.usage = usage,
                Ok(Event::Finish { reason }) => collected.finish = Some(reason),
                Ok(_) => {},
                Err(error) => {
                    failure = Some(error);
                    break;
                },
            }
        }
        let Some(error) = failure else {
            inner.governed.step_finished(call, &collected.usage).await;
            return Ok(collected);
        };
        if !retryable(&error, &attempts) {
            return Err(EngineError::from_llm(error));
        }
        backoff(inner, call, &attempts, &error, cancel).await?;
    }
}
