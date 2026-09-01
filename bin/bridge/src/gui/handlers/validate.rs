//! GUI handlers running self-validation and reporting the result.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::IpcReplyPayload;
use crate::gui::{GuiApp, emit};
use crate::{i18n, validate};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_validate_requested(app: &GuiApp, reply_to: ReplyId) {
    app.append_log(i18n::t("validate-running"));
    let proxy = app.proxy.clone();
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
        let report = validate::run(&http).await;
        proxy.send_event(UiEvent::ValidateFinished { report, reply_to });
    });
}

pub(crate) fn on_validate_finished(
    app: &mut GuiApp,
    report: validate::ValidationReport,
    reply_to: ReplyId,
) {
    let rendered = report.rendered();
    // Why: the whole multi-line report used to go in as one entry, where the log's
    // fixed-height rows truncated it with no wrap. The structured lines now reach
    // the setup-health panel; the log gets a result a reader can actually take in.
    let failed = report
        .lines
        .iter()
        .filter(|l| l.level == validate::CheckLevel::Fail)
        .count();
    let warned = report
        .lines
        .iter()
        .filter(|l| l.level == validate::CheckLevel::Warn)
        .count();
    let summary = i18n::t_args(
        "validate-result",
        &[
            ("checks", &report.lines.len().to_string()),
            ("failed", &failed.to_string()),
            ("warned", &warned.to_string()),
        ],
    );
    if report.any_failed {
        app.append_log_error(&summary);
    } else if warned > 0 {
        app.append_log_warn(&summary);
    } else {
        app.append_log(&summary);
    }
    let report_value = json!({
        "any_failed": report.any_failed,
        "rendered": rendered,
        "lines": report.lines,
    });
    app.state.set_validation(report);
    app.refresh_ui();
    emit::emit_state(app);
    if let Some(id) = reply_to {
        let payload = IpcReplyPayload::ok(json!({ "report": report_value }));
        emit::send_reply_payload(app, id, &payload);
    }
}
