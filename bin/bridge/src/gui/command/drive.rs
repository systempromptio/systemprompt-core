//! Drive commands: the current status and a request to sync now.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

use crate::gui::GuiApp;
use crate::wire::drive::DriveStatusPayload;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::CommandOutcome;

// JSON: webview IPC reply — the typed payload is serialized at the call site.
pub(super) fn drive_dispatch(app: &GuiApp, cmd: &str) -> Option<CommandOutcome> {
    Some(match cmd {
        "drive.status" => {
            let status = app.ctx.drive.status();
            let payload = DriveStatusPayload::from(status.as_ref());
            CommandOutcome::Sync(serde_json::to_value(payload).map_err(|e| {
                BridgeError::from_error_in(
                    ErrorScope::Internal,
                    ErrorCode::Internal,
                    "drive status encode failed",
                    &e,
                )
            }))
        },
        "drive.sync_now" => {
            app.ctx.drive.sync_now();
            CommandOutcome::Sync(Ok(Value::Null))
        },
        _ => return None,
    })
}
