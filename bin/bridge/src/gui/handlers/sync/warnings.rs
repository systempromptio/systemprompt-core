//! Activity-log lines for the warnings a sync reports.
//!
//! A sync repeats a warning for as long as its cause holds; the log records it
//! when it appears, and the Setup health table shows the latest report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::GuiApp;
use crate::sync;

pub(super) fn summary_warnings(summary: &sync::SyncSummary) -> Vec<String> {
    summary
        .host_warnings
        .iter()
        .map(|w| format!("[{}] {}", w.host_id, w.message))
        .collect()
}

pub(super) fn log_host_warnings(app: &GuiApp, lines: Vec<String>) {
    if !app
        .ctx
        .warn_once
        .changed("sync.host_warnings", &lines.join("\n"))
    {
        return;
    }
    for line in lines {
        app.append_log_warn(line);
    }
}
