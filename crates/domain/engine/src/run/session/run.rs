//! The run loop: one driver per busy session that takes turns until its inbox
//! is empty (an interrupted run stores the steers it held for the next turn
//! and drops its queued prompts), and the turn itself, which takes steps until
//! the model stops, the agent's step limit is reached, or the run is aborted.
//! Each step's answer is capped by the agent's output limit, held under the
//! model's ceiling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::config::{AgentDef, Loaded, Ruleset};
use crate::llm::{LlmError, Message, ModelSpec, Request};
use crate::prompt::notices;
use crate::store::{MessageId, MessagePage, MessageWithParts, PartBody, Session, SessionId};
use systemprompt_traits::CancellationToken;

use super::{admit, compaction, history, meta, step, system};
use crate::run::directory::DirState;
use crate::run::error::EngineError;
use crate::run::events::{EngineEvent, RunStatus, SessionFailure};
use crate::run::governor::SessionEvent;
use crate::run::inner::Inner;
use crate::run::tools::registry::{self, ToolBox};

const DEFAULT_MAX_STEPS: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnMode {
    Normal,
    Compact,
}

pub(crate) struct StepContext {
    pub(crate) inner: Arc<Inner>,
    pub(crate) session: Session,
    pub(crate) dir: Arc<DirState>,
    pub(crate) config: Arc<Loaded>,
    pub(crate) agent: AgentDef,
    pub(crate) model: ModelSpec,
    pub(crate) rules: Ruleset,
    pub(crate) cancel: CancellationToken,
}

struct Prepared {
    ctx: StepContext,
    toolbox: ToolBox,
    request: Request,
    parent: MessageId,
    max_steps: u32,
    used: u64,
}

pub(crate) async fn drive(inner: Arc<Inner>, session_id: SessionId, mode: TurnMode) {
    inner.emit(EngineEvent::Status {
        session: session_id.clone(),
        status: RunStatus::Busy,
    });
    let mut next = Some(mode);
    loop {
        if let Some(mode) = next.take()
            && let Some(cancel) = inner.runs.cancel_token(&session_id)
        {
            let result = match mode {
                TurnMode::Normal => turn(&inner, &session_id, &cancel).await,
                TurnMode::Compact => compact_now(&inner, &session_id, &cancel).await,
            };
            if let Err(error) = result {
                report(&inner, &session_id, &error);
            }
            if cancel.is_cancelled()
                && let Err(error) = admit::deliver_steers(&inner, &session_id).await
            {
                report(&inner, &session_id, &error);
            }
        }
        let Some(admission) = inner.runs.finish_or_next(&session_id, &inner.events) else {
            break;
        };
        match admit::write_user(&inner, &session_id, &admission, false).await {
            Ok(_) => next = Some(TurnMode::Normal),
            Err(error) => report(&inner, &session_id, &error),
        }
    }
    tracing::debug!(session = %session_id, "session idle");
    inner
        .governed
        .session_event(&SessionEvent::Idle { session_id });
}

pub(crate) async fn start_compaction(
    inner: &Arc<Inner>,
    session_id: &SessionId,
) -> Result<(), EngineError> {
    inner.session(session_id).await?;
    if !inner.runs.begin(session_id) {
        return Err(EngineError::Busy {
            id: session_id.clone(),
        });
    }
    let run = drive(Arc::clone(inner), session_id.clone(), TurnMode::Compact);
    inner.tasks.spawn("session compaction", run);
    Ok(())
}

fn report(inner: &Inner, session_id: &SessionId, error: &EngineError) {
    if error.is_aborted() {
        tracing::debug!(session = %session_id, "run aborted");
        return;
    }
    tracing::warn!(
        session = %session_id,
        kind = ?error.kind(),
        error = %error.describe(),
        "run failed"
    );
    let failure = SessionFailure::from(error);
    inner.emit(EngineEvent::SessionError {
        session: session_id.clone(),
        error: failure,
    });
}

async fn turn(
    inner: &Arc<Inner>,
    session_id: &SessionId,
    cancel: &CancellationToken,
) -> Result<(), EngineError> {
    let mut steps = 0u32;
    let mut compacted = false;
    loop {
        if cancel.is_cancelled() {
            return Err(EngineError::Aborted);
        }
        admit::deliver_steers(inner, session_id).await?;
        let Prepared {
            ctx,
            toolbox,
            mut request,
            parent,
            max_steps,
            used,
        } = prepare(inner, session_id, cancel).await?;
        if !compacted && compaction::needed(&ctx, &request, used) {
            compacted = true;
            if compaction::run(&ctx, true).await? {
                continue;
            }
        }
        steps += 1;
        let last = steps >= max_steps;
        if last {
            request.tools.clear();
            request
                .messages
                .push(Message::user_text(notices::MAX_STEPS));
        }
        match step::run(&ctx, &toolbox, request, &parent).await {
            Ok(outcome) => {
                if !outcome.more || last {
                    return Ok(());
                }
            },
            Err(error) if !compacted && is_overflow(&error) => {
                compacted = true;
                tracing::info!(session = %session_id, "context overflow; compacting and retrying");
                if !compaction::run(&ctx, true).await? {
                    return Err(error);
                }
                steps = steps.saturating_sub(1);
            },
            Err(error) => return Err(error),
        }
    }
}

const fn is_overflow(error: &EngineError) -> bool {
    matches!(error, EngineError::Model(LlmError::ContextOverflow { .. }))
}

async fn compact_now(
    inner: &Arc<Inner>,
    session_id: &SessionId,
    cancel: &CancellationToken,
) -> Result<(), EngineError> {
    let (ctx, _) = context(inner, session_id, cancel).await?;
    compaction::run(&ctx, false).await.map(|_| ())
}

pub(crate) async fn context(
    inner: &Arc<Inner>,
    session_id: &SessionId,
    cancel: &CancellationToken,
) -> Result<(StepContext, Vec<MessageWithParts>), EngineError> {
    let session = inner.session(session_id).await?;
    let dir = inner.dir(&session.directory).await?;
    let config = dir.current(inner).await?;
    let key = session.id.clone();
    let mut history = inner
        .db
        .run(move |store| store.messages(&key, &MessagePage::all()))
        .await?;
    if let Some(marker) = &session.revert {
        history.retain(|message| message.message.id() < &marker.message_id);
    }
    let last_user = history
        .iter()
        .rev()
        .find_map(|message| message.message.as_user())
        .ok_or(EngineError::EmptyPrompt)?;
    let agent = config
        .agent(&last_user.agent)
        .filter(|agent| !agent.hidden)
        .or_else(|| config.default_agent())
        .cloned()
        .ok_or_else(|| EngineError::UnknownAgent {
            name: last_user.agent.clone(),
        })?;
    let model = inner.resolve_model(Some(&last_user.model))?;
    let rules = inner.session_rules(&session.id).await?;
    let inner = Arc::clone(inner);
    let cancel = cancel.clone();
    let ctx = StepContext {
        inner,
        session,
        dir,
        config,
        agent,
        model,
        rules,
        cancel,
    };
    Ok((ctx, history))
}

async fn prepare(
    inner: &Arc<Inner>,
    session_id: &SessionId,
    cancel: &CancellationToken,
) -> Result<Prepared, EngineError> {
    let (ctx, history) = context(inner, session_id, cancel).await?;
    let parent = history
        .iter()
        .rev()
        .find_map(|message| message.message.as_user().map(|user| user.id.clone()))
        .ok_or(EngineError::EmptyPrompt)?;
    let toolbox = registry::assemble(&ctx);
    let mark = meta::compaction(&ctx.session);
    let mut request = Request::new(ctx.model.clone());
    request.system = system::prompt(&ctx, &toolbox, &history);
    request.messages = history::build(&history, mark.as_ref(), &meta::steered(&ctx.session));
    request.tools = toolbox.defs();
    request.params.temperature = ctx.agent.temperature.map(f64::from);
    request.params.max_output = output_limit(&ctx.agent, &ctx.model);
    let max_steps = ctx
        .agent
        .steps
        .filter(|steps| *steps > 0)
        .unwrap_or(DEFAULT_MAX_STEPS);
    let used = last_usage(&history);
    Ok(Prepared {
        ctx,
        toolbox,
        request,
        parent,
        max_steps,
        used,
    })
}

fn output_limit(agent: &AgentDef, model: &ModelSpec) -> Option<u32> {
    let wanted = agent.max_output.filter(|limit| *limit > 0)?;
    Some(match model.max_output {
        0 => wanted,
        ceiling => wanted.min(ceiling),
    })
}

fn last_usage(history: &[MessageWithParts]) -> u64 {
    for message in history.iter().rev() {
        let compaction = message
            .parts
            .iter()
            .any(|part| matches!(part.body, PartBody::Compaction { .. }));
        if message.message.as_user().is_some() && compaction {
            return 0;
        }
        let assistant = message
            .message
            .as_assistant()
            .filter(|assistant| !assistant.system);
        if let Some(assistant) = assistant {
            let used = assistant.usage.input.saturating_add(assistant.usage.output);
            if used > 0 {
                return used;
            }
        }
    }
    0
}
