//! Naming a session after its first prompt, with the hidden title agent on the
//! small model when one is offered.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use crate::llm::{Message, Request};
use crate::prompt::agents::TITLE;
use crate::prompt::generate::{TITLE_MAX_CHARS, TITLE_PROMPT, clean_title, title_request};
use crate::store::SessionId;
use harness_tasks::CancellationToken;

use super::model_call;
use crate::run::error::EngineError;
use crate::run::governor::{ModelCall, Purpose};
use crate::run::inner::Inner;

const TITLE_MAX_OUTPUT: u32 = 100;

struct TitleJob {
    session_id: SessionId,
    directory: PathBuf,
    text: String,
    model: String,
}

pub(crate) fn spawn(
    inner: &Arc<Inner>,
    session_id: SessionId,
    directory: PathBuf,
    text: String,
    model: String,
) {
    let engine = Arc::clone(inner);
    let job = TitleJob {
        session_id,
        directory,
        text,
        model,
    };
    inner
        .tasks
        .spawn_cancellable("session title", move |cancel| async move {
            if let Err(error) = generate(&engine, &job, &cancel).await {
                let session = &job.session_id;
                let error = error.describe();
                tracing::debug!(session = %session, error = %error, "title generation failed");
            }
        });
}

async fn generate(
    inner: &Inner,
    job: &TitleJob,
    cancel: &CancellationToken,
) -> Result<(), EngineError> {
    let config = inner.config(&job.directory).await?;
    let chosen = config
        .settings
        .small_model
        .clone()
        .filter(|small| inner.is_offered(small))
        .unwrap_or_else(|| job.model.clone());
    let prompt = config
        .agent(TITLE.name)
        .and_then(|agent| agent.prompt.clone())
        .unwrap_or_else(|| TITLE_PROMPT.to_owned());
    let generated = match inner.resolve_model(Some(&chosen)) {
        Ok(spec) => {
            let mut request = Request::new(spec);
            request.system = vec![prompt];
            request.messages = vec![Message::user_text(title_request(&job.text))];
            request.params.max_output = Some(TITLE_MAX_OUTPUT);
            ask_model(inner, job, chosen, &request, cancel).await
        },
        Err(error) => {
            tracing::debug!(session = %job.session_id, error = %error, "no model for the title");
            None
        },
    };
    let Some(title) = generated.or_else(|| fallback(&job.text)) else {
        return Ok(());
    };
    let key = job.session_id.clone();
    inner
        .db
        .run(move |store| {
            store.update_session(&key, |session| {
                if session.title.is_empty() {
                    session.title = title;
                }
            })
        })
        .await?;
    Ok(())
}

async fn ask_model(
    inner: &Inner,
    job: &TitleJob,
    model_id: String,
    request: &Request,
    cancel: &CancellationToken,
) -> Option<String> {
    let call = ModelCall {
        session_id: job.session_id.clone(),
        model_id,
        purpose: Purpose::Title,
        directory: job.directory.clone(),
    };
    match model_call::collect_text(inner, &call, request, cancel).await {
        Ok(collected) => clean_title(&collected.text),
        Err(error) => {
            let session = &job.session_id;
            tracing::debug!(session = %session, error = %error.describe(), "title call failed");
            None
        },
    }
}

fn fallback(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(
        collapsed
            .chars()
            .take(TITLE_MAX_CHARS)
            .collect::<String>()
            .trim_end()
            .to_owned(),
    )
}
