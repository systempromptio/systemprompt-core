//! Diagnostics and proxy-secret command dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use super::{CommandOutcome, send};
use crate::gui::GuiApp;
use crate::gui::events::{ReplyId, UiEvent};

pub(super) fn diagnostics_dispatch(
    app: &GuiApp,
    cmd: &str,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "diagnostics.openLogDirectory" | "openLogFolder" => {
            send(app, UiEvent::OpenLogDirectory { reply_to: reply_id });
            CommandOutcome::Async
        },
        "diagnostics.exportBundle" => {
            send(app, UiEvent::ExportDiagnosticBundle { reply_to: reply_id });
            CommandOutcome::Async
        },
        "proxy.resetSecret" => {
            send(
                app,
                UiEvent::ProxySecretResetRequested { reply_to: reply_id },
            );
            CommandOutcome::Async
        },
        "config.repairDir" => {
            send(
                app,
                UiEvent::ConfigDirRepairRequested { reply_to: reply_id },
            );
            CommandOutcome::Async
        },
        "diagnostics.info" => CommandOutcome::Sync(Ok(json!({
            "version": crate::brand::brand().version,
            "git_sha": crate::buildinfo::short_sha(),
            "git_sha_full": crate::buildinfo::GIT_SHA,
            "build_date": crate::buildinfo::GIT_COMMIT_DATE,
            "build_timestamp": crate::buildinfo::BUILD_TIMESTAMP,
            "branch": crate::buildinfo::GIT_BRANCH,
            "rendered": crate::buildinfo::render(),
        }))),
        _ => return None,
    })
}
