//! Compaction: when the context nears the model's window (or on request), the
//! hidden compaction agent condenses the history into a summary. A boundary
//! message and the summary are stored; the history after them is the summary, a
//! verbatim tail of the most recent exchange, and whatever follows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use crate::llm::tokens::{estimate_message, estimate_request};
use crate::llm::{LlmError, Message, Request};
use crate::prompt::agents::COMPACTION;
use crate::prompt::compaction::{
    COMPACTION_PROMPT, CONTINUE_AFTER_COMPACTION, request as compaction_request,
};
use crate::store::{
    AssistantMessage, FinishReason, MessageId, MessagePage, MessageWithParts, Part, PartBody,
    UserMessage, now_ms,
};

use super::history;
use super::meta::{self, CompactionMark};
use super::model_call::{self, Collected};
use super::run::StepContext;
use super::step::{put_message, put_part, store_usage};
use crate::run::error::EngineError;
use crate::run::governor::{ModelCall, Purpose};
use systemprompt_identifiers::ModelId;

const MIN_HEADROOM: u64 = 16_000;
const HEADROOM_PERCENT: u64 = 15;
const TAIL_PERCENT: u64 = 30;

pub(crate) fn needed(ctx: &StepContext, request: &Request, used: u64) -> bool {
    let settings = &ctx.config.settings.compaction;
    if !settings.auto {
        return false;
    }
    let window = u64::from(ctx.model.context_window);
    if window == 0 {
        return false;
    }
    let reserved = settings
        .reserved_tokens
        .map_or_else(
            || (window * HEADROOM_PERCENT / 100).max(MIN_HEADROOM),
            u64::from,
        )
        .min(window / 2);
    let estimated = estimate_request(request).max(used);
    estimated.saturating_add(reserved) >= window
}

pub(crate) async fn run(ctx: &StepContext, auto: bool) -> Result<bool, EngineError> {
    let session_id = ctx.session.id.clone();
    let key = session_id.clone();
    let all = ctx
        .inner
        .db
        .run(move |store| store.messages(&key, &MessagePage::all()))
        .await?;
    let mark = meta::compaction(&ctx.session);
    let (previous, live) = history::live(&all, mark.as_ref());
    let steered = meta::steered(&ctx.session);
    let window = u64::from(ctx.model.context_window);
    let tail = if auto {
        tail_start(&live, window, &steered)
    } else {
        live.len()
    };
    if tail == 0 {
        return Ok(false);
    }
    let request = summary_request(ctx, &live[..tail], previous.as_deref(), &steered);
    let boundary = write_boundary(ctx, auto).await?;
    let Some(collected) = summarise(ctx, &request, &boundary).await? else {
        return Ok(false);
    };
    write_summary(ctx, &boundary, &collected).await?;
    let tail_id = live.get(tail).map(|message| message.message.id().clone());
    let mark = CompactionMark {
        message: boundary,
        tail: tail_id,
    };
    meta::set_compaction(&ctx.inner.db, &session_id, &mark).await?;
    if auto {
        write_resume(ctx).await?;
    }
    let kept = live.len() - tail;
    tracing::info!(session = %session_id, auto, condensed = tail, kept, "session compacted");
    Ok(true)
}

fn summary_request(
    ctx: &StepContext,
    condensed: &[&MessageWithParts],
    previous: Option<&str>,
    steered: &HashSet<MessageId>,
) -> Request {
    let condensed: Vec<MessageWithParts> =
        condensed.iter().map(|message| (*message).clone()).collect();
    let mut messages = history::map_messages(&condensed, steered);
    messages.push(Message::user_text(compaction_request(previous)));
    let prompt = ctx
        .config
        .agent(COMPACTION.name)
        .and_then(|agent| agent.prompt.clone())
        .unwrap_or_else(|| COMPACTION_PROMPT.to_owned());
    let mut request = Request::new(ctx.model.clone());
    request.system = vec![prompt];
    request.messages = messages;
    request
}

async fn write_boundary(ctx: &StepContext, auto: bool) -> Result<MessageId, EngineError> {
    let session_id = ctx.session.id.clone();
    let mut boundary = UserMessage::new(
        ctx.inner.store.ids(),
        session_id.clone(),
        COMPACTION.name,
        &ctx.model.id,
    );
    boundary.system = true;
    let boundary_id = boundary.id.clone();
    let stored = crate::store::Message::User(boundary);
    ctx.inner
        .db
        .run(move |store| store.upsert_message(&stored))
        .await?;
    let part = Part::new(
        ctx.inner.store.ids(),
        session_id,
        boundary_id.clone(),
        PartBody::Compaction { auto },
    );
    put_part(&ctx.inner, part).await?;
    Ok(boundary_id)
}

async fn summarise(
    ctx: &StepContext,
    request: &Request,
    boundary: &MessageId,
) -> Result<Option<Collected>, EngineError> {
    let call = ModelCall {
        session_id: ctx.session.id.clone(),
        model_id: ModelId::new(ctx.model.id.as_str()),
        purpose: Purpose::Compaction,
        directory: ctx.session.directory.clone(),
    };
    let outcome = model_call::collect_text(&ctx.inner, &call, request, &ctx.cancel)
        .await
        .and_then(refuse_cut_off);
    if let Ok(collected) = &outcome
        && !collected.text.trim().is_empty()
    {
        return outcome.map(Some);
    }
    let removed = boundary.clone();
    if let Err(error) = ctx
        .inner
        .db
        .run(move |store| store.delete_message(&removed))
        .await
    {
        tracing::warn!(session = %ctx.session.id, error = %error, "unfinished compaction kept");
    }
    match outcome {
        Ok(_) => {
            tracing::warn!(session = %ctx.session.id, "compaction produced no summary");
            Ok(None)
        },
        Err(error) => Err(error),
    }
}

fn refuse_cut_off(collected: Collected) -> Result<Collected, EngineError> {
    if !collected.cut_off() {
        return Ok(collected);
    }
    let reason = match collected.finish {
        Some(crate::llm::FinishReason::ContentFilter) => "a content filter",
        _ => "the output limit",
    };
    Err(EngineError::Model(LlmError::Provider {
        kind: None,
        message: format!("the compaction summary stopped at {reason} before it was finished"),
    }))
}

async fn write_summary(
    ctx: &StepContext,
    boundary: &MessageId,
    collected: &Collected,
) -> Result<(), EngineError> {
    let session_id = ctx.session.id.clone();
    let mut summary = AssistantMessage::new(
        ctx.inner.store.ids(),
        session_id.clone(),
        boundary.clone(),
        COMPACTION.name,
        &ctx.model.id,
    );
    summary.system = true;
    summary.usage = store_usage(&collected.usage);
    summary.finish = Some(FinishReason::Stop);
    summary.completed_at = Some(now_ms());
    put_message(&ctx.inner, &summary).await?;
    let text = PartBody::Text {
        text: collected.text.trim().to_owned(),
        synthetic: false,
    };
    put_part(
        &ctx.inner,
        Part::new(ctx.inner.store.ids(), session_id, summary.id.clone(), text),
    )
    .await
}

async fn write_resume(ctx: &StepContext) -> Result<(), EngineError> {
    let session_id = ctx.session.id.clone();
    let mut resume = UserMessage::new(
        ctx.inner.store.ids(),
        session_id.clone(),
        &ctx.agent.name,
        &ctx.model.id,
    );
    resume.system = true;
    let resume_id = resume.id.clone();
    let stored = crate::store::Message::User(resume);
    ctx.inner
        .db
        .run(move |store| store.upsert_message(&stored))
        .await?;
    let body = PartBody::Text {
        text: CONTINUE_AFTER_COMPACTION.to_owned(),
        synthetic: true,
    };
    put_part(
        &ctx.inner,
        Part::new(ctx.inner.store.ids(), session_id, resume_id, body),
    )
    .await
}

fn tail_start(live: &[&MessageWithParts], window: u64, steered: &HashSet<MessageId>) -> usize {
    let Some(start) = live
        .iter()
        .rposition(|message| message.message.as_user().is_some_and(|user| !user.system))
    else {
        return live.len();
    };
    if start == 0 {
        return live.len();
    }
    let tail: Vec<MessageWithParts> = live[start..]
        .iter()
        .map(|message| (*message).clone())
        .collect();
    let tokens: u64 = history::map_messages(&tail, steered)
        .iter()
        .map(estimate_message)
        .sum();
    if window > 0 && tokens > window * TAIL_PERCENT / 100 {
        return live.len();
    }
    start
}
