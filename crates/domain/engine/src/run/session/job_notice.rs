//! Background job notices: a finished job is admitted to its session as a
//! system prompt, the same way a user's prompt is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::admit::admit;
use super::{Admission, Delivery};
use crate::prompt::notices::{self, BackgroundJob};
use crate::run::error::EngineError;
use crate::run::inner::Inner;
use crate::store::{MessagePage, PartBody};
use crate::tools::{JobReport, JobState};

const TRUNCATED_TAIL_BYTES: usize = 8 * 1024;

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
