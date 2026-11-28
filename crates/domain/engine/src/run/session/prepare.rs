//! Preparing one step's request: the context, the parent prompt, the tools,
//! the system prompt and history, the output limit held under the model's
//! ceiling, the step limit, and the context used by the last answer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::config::AgentDef;
use crate::llm::{ModelSpec, Request};
use crate::store::{MessageId, MessageWithParts, PartBody, SessionId};
use systemprompt_traits::CancellationToken;

use super::run::{StepContext, context};
use super::{history, meta, system};
use crate::run::error::EngineError;
use crate::run::inner::Inner;
use crate::run::tools::registry::{self, ToolBox};

const DEFAULT_MAX_STEPS: u32 = 100;

pub(super) struct Prepared {
    pub(super) ctx: StepContext,
    pub(super) toolbox: ToolBox,
    pub(super) request: Request,
    pub(super) parent: MessageId,
    pub(super) max_steps: u32,
    pub(super) used: u64,
}


pub(super) async fn prepare(
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
