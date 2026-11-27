//! One headless session through the same engine and governor as the window:
//! the `run` command.
//!
//! The composition is started, the active account's instance is awaited until
//! the sync applied it, a session is created in the workspace and each prompt
//! is admitted once the one before it left the session idle. A run stops at
//! the first prompt that does not complete. The event stream always ends with
//! a `finished` line carrying the status the process exits with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use systemprompt_engine::run::{Admitted, Delivery, PromptInput};
use systemprompt_engine::store::{Session, SessionId};
use systemprompt_traits::CancellationToken;
use tokio::time::Instant;

use crate::command::compose::Composition;
use crate::command::control::{Control, ControlFeed};
use crate::command::event::{EventWriter, RunEvent};
use crate::command::exit::ExitStatus;
use crate::command::host::HeadlessHost;
use crate::command::permission::{Answers, Decider, PermissionMode};
use crate::command::pump::{Feeds, Limits, Pump, PumpError};
use crate::command::stop::{Stop, describe, ready};

pub const DEFAULT_SYNC_TIMEOUT: Duration = Duration::from_secs(30);

/// One headless run, with its files already read.
///
/// `workspace` is an absolute directory. `sync_timeout` bounds the wait for
/// the instance to be applied; `timeout`, when set, bounds the whole run,
/// which is then aborted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub prompts: Vec<String>,
    pub workspace: PathBuf,
    pub permission_mode: PermissionMode,
    pub answers: Answers,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub data_root: Option<PathBuf>,
    pub sync_timeout: Duration,
    pub timeout: Option<Duration>,
}

/// How a run ended: the same status, session and message as its `finished`
/// line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub status: ExitStatus,
    pub session_id: Option<SessionId>,
    pub message: Option<String>,
}

struct Parts<'a> {
    composition: &'a Composition,
    host: &'a HeadlessHost,
    request: &'a RunRequest,
    cancel: &'a CancellationToken,
}

pub async fn run<W: Write + Send>(
    host: &HeadlessHost,
    request: RunRequest,
    cancel: &CancellationToken,
    out: &mut EventWriter<W>,
) -> RunReport {
    Box::pin(conduct(host, &request, (cancel, None), out)).await
}

pub async fn run_controlled<W: Write + Send>(
    host: &HeadlessHost,
    request: RunRequest,
    control: (&CancellationToken, ControlFeed),
    out: &mut EventWriter<W>,
) -> RunReport {
    let (cancel, controls) = control;
    Box::pin(conduct(host, &request, (cancel, Some(controls)), out)).await
}

async fn conduct<W: Write + Send>(
    host: &HeadlessHost,
    request: &RunRequest,
    control: (&CancellationToken, Option<ControlFeed>),
    out: &mut EventWriter<W>,
) -> RunReport {
    let (cancel, controls) = control;
    let root = host.root(request.data_root.as_deref());
    let mut decider = Decider::new(request.permission_mode, request.answers.clone());
    let report = match Composition::start(host, &root) {
        Ok(composition) => {
            let parts = Parts {
                composition: &composition,
                host,
                request,
                cancel,
            };
            let report = Box::pin(drive(&parts, controls, &mut decider, out)).await;
            composition.shutdown().await;
            report
        },
        Err(error) => Stop::new(ExitStatus::Internal, describe(&error)).report(None),
    };
    let finished = RunEvent::Finished {
        session_id: report.session_id.clone(),
        outcome: report.status,
        exit_code: report.status.code(),
        message: report.message.clone(),
        unused_answers: decider.unused(),
    };
    if let Err(error) = out.emit(&finished) {
        tracing::warn!(error = %error, "the finished line could not be written");
    }
    report
}

async fn drive<W: Write + Send>(
    parts: &Parts<'_>,
    controls: Option<ControlFeed>,
    decider: &mut Decider,
    out: &mut EventWriter<W>,
) -> RunReport {
    if let Err(stop) = ready(parts.composition, parts.request.sync_timeout, parts.cancel).await {
        return stop.report(None);
    }
    let engine = &parts.composition.engine;
    let feeds = Feeds {
        changes: engine.store().subscribe(),
        events: engine.subscribe(),
        controls,
    };
    let session = match engine.create(&parts.request.workspace).await {
        Ok(session) => session,
        Err(error) => return Stop::of_engine(&error).report(None),
    };
    let id = session.id.clone();
    match converse(parts, feeds, &session, decider, out).await {
        Ok(()) => RunReport {
            status: ExitStatus::Completed,
            session_id: Some(id),
            message: None,
        },
        Err(stop) => stop.report(Some(id)),
    }
}

async fn converse<W: Write + Send>(
    parts: &Parts<'_>,
    feeds: Feeds,
    session: &Session,
    decider: &mut Decider,
    out: &mut EventWriter<W>,
) -> Result<(), Stop> {
    let request = parts.request;
    let limits = Limits {
        cancel: parts.cancel,
        deadline: request.timeout.map(|limit| Instant::now() + limit),
    };
    let mut pump = Pump {
        engine: &parts.composition.engine,
        root: session.id.clone(),
        decider,
        out,
        feeds,
        template: PromptInput {
            session_id: session.id.clone(),
            directory: session.directory.clone(),
            text: String::new(),
            attachments: Vec::new(),
            agent: request.agent.clone(),
            model: request.model.clone(),
            delivery: Delivery::Queue,
        },
        failure: None,
        interrupted: None,
        titled: !session.title.is_empty(),
    };
    pump.emit(&RunEvent::Started {
        session_id: session.id.clone(),
        directory: session.directory.clone(),
        permission_mode: request.permission_mode,
        product: parts.host.info.product.to_owned(),
        version: parts.host.info.version.to_owned(),
    })
    .map_err(Stop::of_pump)?;
    for (index, text) in request.prompts.iter().enumerate() {
        turn(&mut pump, text.clone(), index == 0, &limits).await?;
    }
    loop {
        let control = pump.while_idle(&limits).await.map_err(Stop::of_pump)?;
        match control {
            None | Some(Control::Close) => break,
            Some(Control::Prompt { text, .. }) => turn(&mut pump, text, false, &limits).await?,
            Some(control) => pump.during_turn(control).await.map_err(Stop::of_pump)?,
        }
        if pump.interrupted.is_some() {
            break;
        }
    }
    pump.interrupted.map_or(Ok(()), |status| {
        Err(Stop {
            status,
            message: None,
        })
    })
}

async fn turn<W: Write + Send>(
    pump: &mut Pump<'_, W>,
    text: String,
    first: bool,
    limits: &Limits<'_>,
) -> Result<(), Stop> {
    let engine = pump.engine;
    pump.failure = None;
    let input = PromptInput {
        text,
        ..pump.template.clone()
    };
    let admitted = engine
        .prompt(input)
        .await
        .map_err(|error| Stop::of_engine(&error))?;
    if !matches!(admitted, Admitted::Started { .. }) {
        return Err(Stop::new(
            ExitStatus::Internal,
            "the session was busy when a prompt was admitted",
        ));
    }
    let pumped = async {
        pump.until_idle(limits).await?;
        if first {
            pump.settle_title().await?;
        }
        Ok::<(), PumpError>(())
    }
    .await;
    if let Err(error) = pumped {
        engine.abort(&pump.root);
        return Err(Stop::of_pump(error));
    }
    let messages = engine.messages(&pump.root).await;
    let (status, message) = pump.outcome(messages);
    if status.is_success() {
        Ok(())
    } else {
        Err(Stop { status, message })
    }
}
