//! Inbox commands: list, load older, seen, mark read, escalate, open, copy the
//! prompt and start a session. Gateway work is queued on the inbox task, which
//! holds the bearer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::{MarkRead, MarkSeen, Notification};

use crate::gui::GuiApp;
use crate::integration::host_app::{StartSession, session_starter};
use crate::proxy::notifications::InboxRequest;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::{CommandOutcome, parse};

#[derive(Debug, Deserialize)]
struct IdArgs {
    id: NotificationId,
}

#[derive(Debug, Deserialize)]
struct EscalateArgs {
    id: NotificationId,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct PromptReply {
    prompt: String,
}

// JSON: webview IPC args and reply — decoded with `parse::<T>`, typed replies
// serialized here.
pub(super) fn inbox_dispatch(app: &GuiApp, cmd: &str, args: Value) -> Option<CommandOutcome> {
    let result = match cmd {
        "inbox.list" => {
            app.ctx.inbox.request(InboxRequest::Refresh);
            crate::gui::inbox::take_pending_show(app);
            encode(&crate::gui::inbox::payload(app))
        },
        "inbox.load_older" => {
            app.ctx.inbox.request(InboxRequest::LoadOlder);
            Ok(Value::Null)
        },
        "inbox.seen" => parse::<MarkSeen>(args).map(|body| {
            app.ctx.inbox.request(InboxRequest::MarkSeen(body));
            Value::Null
        }),
        "inbox.read" => parse::<MarkRead>(args).map(|body| {
            app.ctx.inbox.request(InboxRequest::MarkRead(body));
            Value::Null
        }),
        "inbox.escalate" => parse::<EscalateArgs>(args).map(|a| {
            app.ctx.inbox.request(InboxRequest::Escalate {
                id: a.id,
                note: a.note.filter(|note| !note.trim().is_empty()),
            });
            Value::Null
        }),
        "inbox.open" => parse::<IdArgs>(args).and_then(|a| open(app, &a.id)),
        "inbox.copy_prompt" => parse::<IdArgs>(args)
            .and_then(|a| item(app, &a.id))
            .and_then(|n| {
                encode(&PromptReply {
                    prompt: n.action.prompt,
                })
            }),
        "inbox.start_session" => parse::<IdArgs>(args)
            .and_then(|a| item(app, &a.id))
            .and_then(|n| start_session(&n.action.prompt)),
        _ => return None,
    };
    Some(CommandOutcome::Sync(result))
}

fn item(app: &GuiApp, id: &NotificationId) -> Result<Notification, BridgeError> {
    app.ctx.inbox.snapshot().find(id).cloned().ok_or_else(|| {
        BridgeError::new(
            ErrorScope::Internal,
            ErrorCode::NotFound,
            format!("notification {id} is not in the inbox"),
        )
    })
}

fn open(app: &GuiApp, id: &NotificationId) -> Result<Value, BridgeError> {
    let notification = item(app, id)?;
    let url = notification
        .action
        .open_url
        .as_deref()
        .and_then(|link| crate::gui::inbox::resolve_open_url(app, link))
        .ok_or_else(|| BridgeError::invalid_args(format!("notification {id} has no link")))?;
    crate::gui::window::open_external_url(&url);
    if notification.read_at.is_none() {
        app.ctx.inbox.request(InboxRequest::MarkRead(MarkRead {
            ids: vec![notification.id],
            all: false,
        }));
    }
    Ok(Value::Null)
}

fn start_session(prompt: &str) -> Result<Value, BridgeError> {
    let unsupported = || {
        BridgeError::new(
            ErrorScope::Host,
            ErrorCode::NotFound,
            "no agent on this device can start a session from a prompt",
        )
    };
    let launch = session_starter()
        .and_then(|host| host.start_session(prompt))
        .ok_or_else(unsupported)?;
    let launched = match launch {
        StartSession::Url(url) => opener::open(url.as_str()).map_err(|e| {
            BridgeError::from_error_in(
                ErrorScope::Host,
                ErrorCode::Internal,
                "session start failed",
                &e,
            )
        }),
        StartSession::Command(argv) => {
            let (program, rest) = argv.split_first().ok_or_else(unsupported)?;
            std::process::Command::new(program)
                .args(rest)
                .spawn()
                .map(|_| ())
                .map_err(|e| {
                    BridgeError::from_error_in(
                        ErrorScope::Host,
                        ErrorCode::Internal,
                        "session start failed",
                        &e,
                    )
                })
        },
    };
    launched.map(|()| Value::Null)
}

fn encode<T: Serialize>(value: &T) -> Result<Value, BridgeError> {
    serde_json::to_value(value).map_err(|e| {
        BridgeError::from_error_in(
            ErrorScope::Internal,
            ErrorCode::Internal,
            "inbox reply encode failed",
            &e,
        )
    })
}
