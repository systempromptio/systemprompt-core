//! Slash commands: the built-ins (compact, undo, redo) and templates from
//! configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::store::{MessageId, MessagePage, SessionId};

use super::{CommandInfo, CommandInput, CommandOutcome, PromptInput, admit, revert};
use crate::run::error::EngineError;
use crate::run::inner::Inner;

const COMPACT: &str = "compact";
const UNDO: &str = "undo";
const REDO: &str = "redo";

pub(crate) fn is_builtin_command(name: &str) -> bool {
    matches!(name, COMPACT | UNDO | REDO)
}

pub(crate) fn builtin_commands() -> Vec<CommandInfo> {
    [
        (
            COMPACT,
            "Condense the conversation so far into a summary to free up context.",
        ),
        (
            UNDO,
            "Revert the last prompt and the file changes made since.",
        ),
        (REDO, "Bring back what the last undo reverted."),
    ]
    .into_iter()
    .map(|(name, description)| CommandInfo {
        name: name.to_owned(),
        description: Some(description.to_owned()),
        builtin: true,
    })
    .collect()
}

pub(crate) async fn run(
    inner: &Arc<Inner>,
    input: CommandInput,
) -> Result<CommandOutcome, EngineError> {
    let name = input.name.trim().trim_start_matches('/').to_owned();
    match name.as_str() {
        COMPACT => {
            super::run::start_compaction(inner, &input.session_id).await?;
            Ok(CommandOutcome::Compacting)
        },
        UNDO => {
            let target = undo_target(inner, &input.session_id).await?;
            let session = revert::revert(inner, &input.session_id, &target).await?;
            Ok(CommandOutcome::Reverted(Box::new(session)))
        },
        REDO => {
            let session = revert::unrevert(inner, &input.session_id).await?;
            Ok(CommandOutcome::Reverted(Box::new(session)))
        },
        _ => template(inner, input, &name).await,
    }
}

async fn template(
    inner: &Arc<Inner>,
    input: CommandInput,
    name: &str,
) -> Result<CommandOutcome, EngineError> {
    let config = inner.config(&input.directory).await?;
    let command = config
        .commands
        .get(name)
        .ok_or_else(|| EngineError::UnknownCommand {
            name: name.to_owned(),
        })?;
    let prompt = PromptInput {
        session_id: input.session_id,
        directory: input.directory.clone(),
        text: command.render(&input.arguments),
        attachments: Vec::new(),
        agent: input.agent.or_else(|| command.agent.clone()),
        model: input.model.or_else(|| command.model.clone()),
        delivery: input.delivery,
    };
    Ok(CommandOutcome::Prompted(
        admit::prompt(inner, prompt).await?,
    ))
}

async fn undo_target(inner: &Inner, session_id: &SessionId) -> Result<MessageId, EngineError> {
    let session = inner.session(session_id).await?;
    let key = session_id.clone();
    let messages = inner
        .db
        .run(move |store| store.messages(&key, &MessagePage::all()))
        .await?;
    let cut = session.revert.map(|marker| marker.message_id);
    messages
        .iter()
        .rev()
        .filter_map(|message| message.message.as_user())
        .filter(|user| !user.system)
        .map(|user| user.id.clone())
        .find(|id| cut.as_ref().is_none_or(|cut| id < cut))
        .ok_or(EngineError::NothingToRevert)
}
