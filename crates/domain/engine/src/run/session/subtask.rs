//! Subagent runs for the task tool: a child session with the chosen agent,
//! driven by the same loop and awaited by the tool call that started it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use harness_config::{AgentDef, Loaded, Ruleset, permission};
use harness_prompt::tool_names::FILE_WRITERS;
use harness_store::{MessagePage, MessageWithParts, PartBody, SessionId};
use harness_tasks::CancellationToken;
use harness_tools::ToolError;

use super::history::text_of;
use super::run::{self, TurnMode};
use super::{Admission, Admit, Delivery, admit, create};
use crate::inner::Inner;

#[derive(Debug, Clone)]
pub(crate) struct SubtaskRequest {
    pub(crate) parent: SessionId,
    pub(crate) directory: PathBuf,
    pub(crate) agent: String,
    pub(crate) description: String,
    pub(crate) prompt: String,
    pub(crate) model: String,
    pub(crate) caller_writes: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SubtaskResult {
    pub(crate) session: SessionId,
    pub(crate) text: String,
}

pub(crate) fn can_write(config: &Loaded, agent: &AgentDef) -> bool {
    let has_writer = FILE_WRITERS.iter().any(|tool| agent.tools.permits(tool));
    has_writer
        && !config
            .rules_for(agent, &Ruleset::new())
            .denies_all(permission::EDIT)
}

pub(crate) async fn run(
    inner: &Arc<Inner>,
    request: SubtaskRequest,
    cancel: &CancellationToken,
    started: impl FnOnce(&SessionId),
) -> Result<SubtaskResult, ToolError> {
    let agent = subagent(inner, &request).await?;
    let offered = agent.model.clone().filter(|model| inner.is_offered(model));
    let model = offered.unwrap_or(request.model);
    let child = create(
        inner,
        &request.directory,
        Some(request.parent.clone()),
        Some(request.description.clone()),
        Some(agent.name.clone()),
    )
    .await
    .map_err(|error| ToolError::caused("could not create the subagent session", error))?;
    started(&child.id);
    let admission = Admission {
        agent: agent.name.clone(),
        model,
        parts: vec![PartBody::Text {
            text: request.prompt.clone(),
            synthetic: false,
        }],
        text: request.prompt,
        system: false,
        delivery: Delivery::Queue,
        directory: request.directory,
    };
    match inner.runs.admit(&child.id, admission, Some(cancel)) {
        Admit::Start(admission) => {
            if let Err(error) = admit::write_user(inner, &child.id, &admission, false).await {
                inner.runs.end(&child.id);
                return Err(ToolError::caused("could not start the subagent", error));
            }
        },
        Admit::Inboxed(_) => {
            return Err(ToolError::failed("the subagent session is already running"));
        },
    }
    run::drive(Arc::clone(inner), child.id.clone(), TurnMode::Normal).await;
    if cancel.is_cancelled() {
        return Err(ToolError::Aborted);
    }
    let key = child.id.clone();
    let messages = inner
        .db
        .run(move |store| store.messages(&key, &MessagePage::latest(20)))
        .await
        .map_err(|error| ToolError::caused("could not read the subagent's answer", error))?;
    answer(child.id, &messages)
}

async fn subagent(inner: &Inner, request: &SubtaskRequest) -> Result<AgentDef, ToolError> {
    let config = inner
        .config(&request.directory)
        .await
        .map_err(|error| ToolError::caused("could not load the subagent configuration", error))?;
    let agent = config
        .agent(&request.agent)
        .filter(|agent| agent.mode.is_subagent() && !agent.hidden)
        .cloned()
        .ok_or_else(|| {
            ToolError::invalid(format!("there is no subagent named {}", request.agent))
        })?;
    if !request.caller_writes && can_write(&config, &agent) {
        let name = &agent.name;
        let reason =
            format!("this agent may not change files, so it cannot start {name}, which can");
        return Err(ToolError::Denied { reason });
    }
    Ok(agent)
}

fn answer(session: SessionId, messages: &[MessageWithParts]) -> Result<SubtaskResult, ToolError> {
    let last = messages.iter().rev().find(|message| {
        message
            .message
            .as_assistant()
            .is_some_and(|assistant| !assistant.system)
            && !text_of(message).trim().is_empty()
    });
    let failure = messages
        .iter()
        .rev()
        .find_map(|message| message.message.as_assistant())
        .and_then(|assistant| assistant.error.clone());
    match (last, failure) {
        (Some(message), _) => Ok(SubtaskResult {
            session,
            text: text_of(message),
        }),
        (None, Some(error)) => Err(ToolError::failed(format!(
            "the subagent failed: {}",
            error.message
        ))),
        (None, None) => Ok(SubtaskResult {
            session,
            text: String::new(),
        }),
    }
}
