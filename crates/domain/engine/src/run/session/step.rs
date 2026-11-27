//! One step: an assistant message bracketed by snapshots, one model call
//! streamed into it, then its tool calls.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use harness_llm::retry::{self, Retry};
use harness_llm::{LlmError, Request};
use harness_store::{
    AssistantMessage, FinishReason, Message, MessageError, MessageErrorKind, MessageId, Part,
    PartBody, Usage, now_ms,
};
use harness_workspace::SnapshotId;

use super::attachments::slash;
use super::run::StepContext;
use super::{revert, stream};
use crate::error::{EngineError, ErrorKind};
use crate::governor::{ModelCall, Purpose};
use crate::inner::Inner;
use crate::tools::exec;
use crate::tools::registry::ToolBox;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StepOutcome {
    pub(crate) more: bool,
}

pub(crate) async fn run(
    ctx: &StepContext,
    toolbox: &ToolBox,
    request: Request,
    parent: &MessageId,
) -> Result<StepOutcome, EngineError> {
    let inner = &ctx.inner;
    let session_id = ctx.session.id.clone();
    let mut message = AssistantMessage::new(
        session_id.clone(),
        parent.clone(),
        &ctx.agent.name,
        &ctx.model.id,
    );
    put_message(inner, &message).await?;
    let before = snapshot(ctx).await;
    let start = PartBody::StepStart {
        snapshot: before.as_ref().map(ToString::to_string),
    };
    put_part(
        inner,
        Part::new(session_id.clone(), message.id.clone(), start),
    )
    .await?;

    let call = ModelCall {
        session_id: session_id.clone(),
        model_id: ctx.model.id.clone(),
        purpose: Purpose::Step,
        directory: ctx.session.directory.clone(),
    };
    let streamed = match stream::consume(ctx, &call, &request, &message.id).await {
        Ok(streamed) => streamed,
        Err(error) => {
            fail(inner, &mut message, &error).await;
            return Err(error);
        },
    };

    let has_calls = !streamed.calls.is_empty();
    let stop = has_calls && exec::run_calls(ctx, toolbox, &message.id, &streamed.calls).await;
    let after = if has_calls {
        snapshot(ctx).await
    } else {
        before.clone()
    };
    let reason = if has_calls {
        FinishReason::ToolCalls
    } else {
        finish_reason(streamed.finish.as_ref())
    };
    let usage = store_usage(&streamed.usage);
    let recorded = after.as_ref().map(ToString::to_string);
    let finish = PartBody::StepFinish {
        reason,
        usage,
        snapshot: recorded,
    };
    put_part(
        inner,
        Part::new(session_id.clone(), message.id.clone(), finish),
    )
    .await?;
    if has_calls && let Some(before) = &before {
        record_patch(ctx, &message.id, before).await;
    }

    complete(ctx, &mut message, &call, &streamed.usage, reason).await?;
    Ok(StepOutcome {
        more: has_calls && !stop,
    })
}

async fn complete(
    ctx: &StepContext,
    message: &mut AssistantMessage,
    call: &ModelCall,
    usage: &harness_llm::Usage,
    reason: FinishReason,
) -> Result<(), EngineError> {
    let aborted = ctx.cancel.is_cancelled();
    message.usage = store_usage(usage);
    message.finish = Some(if aborted {
        FinishReason::Aborted
    } else {
        reason
    });
    message.completed_at = Some(now_ms());
    if aborted {
        message.error = Some(message_error(&EngineError::Aborted));
    }
    put_message(&ctx.inner, message).await?;
    ctx.inner.governed.step_finished(call, usage).await;
    if aborted {
        return Err(EngineError::Aborted);
    }
    Ok(())
}

async fn snapshot(ctx: &StepContext) -> Option<SnapshotId> {
    match ctx.dir.workspace.track().await {
        Ok(id) => Some(id),
        Err(error) => {
            tracing::warn!(session = %ctx.session.id, error = %error, "workspace snapshot failed");
            None
        },
    }
}

async fn record_patch(ctx: &StepContext, message_id: &MessageId, before: &SnapshotId) {
    let files = match ctx.dir.workspace.changed_files(before.clone()).await {
        Ok(files) => files,
        Err(error) => {
            tracing::debug!(session = %ctx.session.id, error = %error, "changed files unavailable");
            return;
        },
    };
    if files.is_empty() {
        return;
    }
    let files: Vec<String> = files.iter().map(|path| slash(path)).collect();
    let patch = PartBody::Patch {
        hash: before.to_string(),
        files,
    };
    let part = Part::new(ctx.session.id.clone(), message_id.clone(), patch);
    if let Err(error) = put_part(&ctx.inner, part).await {
        tracing::warn!(session = %ctx.session.id, error = %error, "could not record a patch");
    }
    revert::refresh_summary(&ctx.inner, &ctx.session.id, &ctx.dir).await;
}

async fn fail(inner: &Inner, message: &mut AssistantMessage, error: &EngineError) {
    message.error = Some(message_error(error));
    message.finish = Some(if error.is_aborted() {
        FinishReason::Aborted
    } else {
        FinishReason::Error
    });
    message.completed_at = Some(now_ms());
    if let Err(store_error) = put_message(inner, message).await {
        tracing::warn!(message = %message.id, error = %store_error, "failed step not recorded");
    }
}

pub(crate) fn message_error(error: &EngineError) -> MessageError {
    let status = match error {
        EngineError::Model(LlmError::Status { status, .. }) => Some(*status),
        EngineError::Model(LlmError::ContextOverflow { status, .. }) => *status,
        _ => None,
    };
    let kind = match error.kind() {
        ErrorKind::Aborted => MessageErrorKind::Aborted,
        ErrorKind::NotSignedIn | ErrorKind::ModelNotOffered | ErrorKind::Governance => {
            MessageErrorKind::Governance
        },
        ErrorKind::ContextOverflow => MessageErrorKind::ContextOverflow,
        ErrorKind::Model | ErrorKind::Unavailable => match status {
            Some(401 | 403) => MessageErrorKind::Auth,
            _ => MessageErrorKind::Api,
        },
        _ => MessageErrorKind::Unknown,
    };
    let retryable = matches!(error, EngineError::Model(llm) if retry::classify(llm) != Retry::No);
    MessageError {
        kind,
        message: error.describe(),
        status,
        retryable,
    }
}

const fn finish_reason(reason: Option<&harness_llm::FinishReason>) -> FinishReason {
    match reason {
        Some(harness_llm::FinishReason::Stop) => FinishReason::Stop,
        Some(harness_llm::FinishReason::ToolCalls) => FinishReason::ToolCalls,
        Some(harness_llm::FinishReason::Length) => FinishReason::Length,
        Some(harness_llm::FinishReason::ContentFilter) => FinishReason::ContentFilter,
        Some(harness_llm::FinishReason::Other(_)) | None => FinishReason::Unknown,
    }
}

pub(crate) const fn store_usage(usage: &harness_llm::Usage) -> Usage {
    Usage {
        input: usage.input,
        output: usage.output,
        reasoning: usage.reasoning,
        cache_read: usage.cache_read,
        cache_write: usage.cache_write,
    }
}

pub(crate) async fn put_message(
    inner: &Inner,
    message: &AssistantMessage,
) -> Result<(), EngineError> {
    let message = Message::Assistant(message.clone());
    inner
        .db
        .run(move |store| store.upsert_message(&message))
        .await
}

pub(crate) async fn put_part(inner: &Inner, part: Part) -> Result<(), EngineError> {
    inner.db.run(move |store| store.upsert_part(&part)).await?;
    Ok(())
}
