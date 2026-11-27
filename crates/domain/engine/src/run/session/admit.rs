//! Admitting prompts: resolving agent and model, writing the user message,
//! telling the Governor, starting a run or holding the prompt in the inbox.
//! Background job notices come in the same way.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use harness_config::{AgentDef, Loaded};
use harness_prompt::notices::{self, BackgroundJob};
use harness_store::{
    Message, MessageId, MessagePage, Part, PartBody, Session, SessionId, UserMessage,
};
use harness_tools::{JobReport, JobState};

use super::run::{self, TurnMode};
use super::{Admission, Admit, Admitted, Delivery, PromptInput, attachments, meta, revert, title};
use crate::error::EngineError;
use crate::governor::UserPrompt;
use crate::inner::Inner;

const TRUNCATED_TAIL_BYTES: usize = 8 * 1024;

pub(crate) async fn prompt(
    inner: &Arc<Inner>,
    input: PromptInput,
) -> Result<Admitted, EngineError> {
    if !input.directory.is_absolute() {
        return Err(EngineError::RelativeDirectory {
            path: input.directory,
        });
    }
    let session = inner.session(&input.session_id).await?;
    let config = inner.config(&input.directory).await?;
    let agent = resolve_agent(&config, input.agent.as_deref().or(session.agent.as_deref()))?;
    let model = resolve_model(inner, &config, &agent, &session, input.model.as_deref())?;
    let mut parts = Vec::new();
    if !input.text.trim().is_empty() {
        parts.push(PartBody::Text {
            text: input.text.clone(),
            synthetic: false,
        });
    }
    parts.extend(attachments::parts(&input.attachments, &input.directory).await?);
    if parts.is_empty() {
        return Err(EngineError::EmptyPrompt);
    }
    if !inner.runs.is_running(&session.id) {
        revert::commit_pending(inner, &session).await?;
    }
    let session = remember_choice(inner, session, &agent.name, &model, &input).await?;
    let admission = Admission {
        agent: agent.name.clone(),
        model,
        parts,
        text: input.text,
        system: false,
        delivery: input.delivery,
        directory: session.directory.clone(),
    };
    admit(inner, &session.id, admission).await
}

async fn remember_choice(
    inner: &Inner,
    session: Session,
    agent: &str,
    model: &str,
    input: &PromptInput,
) -> Result<Session, EngineError> {
    if session.agent.as_deref() == Some(agent)
        && session.model.as_deref() == Some(model)
        && session.directory == input.directory
    {
        return Ok(session);
    }
    let key = session.id.clone();
    let (agent, model, directory) = (agent.to_owned(), model.to_owned(), input.directory.clone());
    inner
        .db
        .run(move |store| {
            store.update_session(&key, |session| {
                session.agent = Some(agent);
                session.model = Some(model);
                session.directory = directory;
            })
        })
        .await
}

fn resolve_agent(config: &Loaded, requested: Option<&str>) -> Result<AgentDef, EngineError> {
    let Some(name) = requested.map(str::trim).filter(|name| !name.is_empty()) else {
        return config
            .default_agent()
            .cloned()
            .ok_or_else(|| EngineError::UnknownAgent {
                name: String::new(),
            });
    };
    config
        .agent(name)
        .filter(|agent| !agent.hidden)
        .cloned()
        .ok_or_else(|| EngineError::UnknownAgent {
            name: name.to_owned(),
        })
}

fn resolve_model(
    inner: &Inner,
    config: &Loaded,
    agent: &AgentDef,
    session: &Session,
    requested: Option<&str>,
) -> Result<String, EngineError> {
    if let Some(id) = requested.map(str::trim).filter(|id| !id.is_empty()) {
        return inner.resolve_model(Some(id)).map(|model| model.id);
    }
    let fallbacks = [
        agent.model.as_deref(),
        session.model.as_deref(),
        config.settings.model.as_deref(),
    ];
    if let Some(id) = fallbacks
        .into_iter()
        .flatten()
        .find(|id| inner.is_offered(id))
    {
        return Ok(id.to_owned());
    }
    inner.resolve_model(None).map(|model| model.id)
}

pub(crate) async fn admit(
    inner: &Arc<Inner>,
    session_id: &SessionId,
    admission: Admission,
) -> Result<Admitted, EngineError> {
    match inner.runs.admit(session_id, admission, None) {
        Admit::Inboxed(Delivery::Steer) => Ok(Admitted::Steered),
        Admit::Inboxed(Delivery::Queue) => Ok(Admitted::Queued),
        Admit::Start(admission) => {
            let message_id = match write_user(inner, session_id, &admission, false).await {
                Ok(id) => id,
                Err(error) => {
                    inner.runs.end(session_id);
                    return Err(error);
                },
            };
            let run = run::drive(Arc::clone(inner), session_id.clone(), TurnMode::Normal);
            inner.tasks.spawn("session run", run);
            Ok(Admitted::Started { message_id })
        },
    }
}

pub(crate) async fn deliver_steers(
    inner: &Arc<Inner>,
    session_id: &SessionId,
) -> Result<(), EngineError> {
    for admission in inner.runs.take_steers(session_id) {
        let steered = !admission.system;
        write_user(inner, session_id, &admission, steered).await?;
    }
    Ok(())
}

pub(crate) async fn write_user(
    inner: &Arc<Inner>,
    session_id: &SessionId,
    admission: &Admission,
    steered: bool,
) -> Result<MessageId, EngineError> {
    let mut message = UserMessage::new(session_id.clone(), &admission.agent, &admission.model);
    message.system = admission.system;
    let message_id = message.id.clone();
    let parts: Vec<Part> = admission
        .parts
        .iter()
        .map(|body| Part::new(session_id.clone(), message_id.clone(), body.clone()))
        .collect();
    let stored = Message::User(message);
    inner
        .db
        .run(move |store| {
            store.upsert_message(&stored)?;
            for part in &parts {
                store.upsert_part(part)?;
            }
            Ok(())
        })
        .await?;
    if steered {
        meta::mark_steered(&inner.db, session_id, &message_id).await?;
    }
    if !admission.system {
        let prompt = UserPrompt {
            session_id: session_id.clone(),
            message_id: message_id.clone(),
            text: admission.text.clone(),
            agent: admission.agent.clone(),
            model_id: admission.model.clone(),
            directory: admission.directory.clone(),
        };
        inner.governed.user_prompt(&prompt).await;
        maybe_title(inner, session_id, admission).await;
    }
    Ok(message_id)
}

async fn maybe_title(inner: &Arc<Inner>, session_id: &SessionId, admission: &Admission) {
    let Ok(session) = inner.session(session_id).await else {
        return;
    };
    if !session.title.is_empty() || session.parent_id.is_some() || admission.text.trim().is_empty()
    {
        return;
    }
    let key = session_id.clone();
    let recent = inner
        .db
        .run(move |store| store.messages(&key, &MessagePage::latest(8)))
        .await;
    let Ok(messages) = recent else {
        return;
    };
    let prompts = messages
        .iter()
        .filter(|message| message.message.as_user().is_some_and(|user| !user.system))
        .count();
    if prompts == 1 {
        let (text, model) = (admission.text.clone(), admission.model.clone());
        title::spawn(inner, session_id.clone(), session.directory, text, model);
    }
}

pub(crate) async fn job_finished(inner: &Arc<Inner>, report: JobReport) -> Result<(), EngineError> {
    let session_id = report.job.session_id.clone();
    let key = session_id.clone();
    let Some(session) = inner.db.run(move |store| store.get_session(&key)).await? else {
        return Ok(());
    };
    let key = session_id.clone();
    let recent = inner
        .db
        .run(move |store| store.messages(&key, &MessagePage::latest(50)))
        .await?;
    let last = recent
        .iter()
        .rev()
        .find_map(|message| message.message.as_user());
    let agent = last
        .map(|user| user.agent.clone())
        .or_else(|| session.agent.clone());
    let model = last
        .map(|user| user.model.clone())
        .or_else(|| session.model.clone());
    let (Some(agent), Some(model)) = (agent, model) else {
        tracing::debug!(
            session = %session_id,
            job = %report.job.id,
            "background job ended in a session without prompts"
        );
        return Ok(());
    };
    let notice = notices::background_job_finished(&BackgroundJob {
        id: &report.job.id,
        command: &report.job.command,
        exit_code: exit_code(&report),
        output: &report.tail,
        output_truncated: report.tail.len() >= TRUNCATED_TAIL_BYTES,
    });
    let admission = Admission {
        agent,
        model,
        parts: vec![PartBody::Text {
            text: notice.clone(),
            synthetic: true,
        }],
        text: notice,
        system: true,
        delivery: Delivery::Steer,
        directory: session.directory.clone(),
    };
    admit(inner, &session_id, admission).await?;
    Ok(())
}

const fn exit_code(report: &JobReport) -> Option<i32> {
    match report.job.state {
        JobState::Exited => report.job.exit_code,
        JobState::Running | JobState::TimedOut | JobState::Killed => None,
    }
}
