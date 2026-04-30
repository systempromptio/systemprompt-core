use serde_json::json;

use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::IpcReplyPayload;
use crate::gui::{GuiApp, ipc_runtime};
use crate::{i18n, validate};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_validate_requested(app: &mut GuiApp, reply_to: ReplyId) {
    app.append_log(i18n::t("validate-running"));
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let report = validate::run().await;
        let _ = proxy.send_event(UiEvent::ValidateFinished { report, reply_to });
    });
}

pub(crate) fn on_validate_finished(
    app: &mut GuiApp,
    report: validate::ValidationReport,
    reply_to: ReplyId,
) {
    let rendered = report.rendered();
    app.append_log(&rendered);
    let report_value = json!({
        "any_failed": report.any_failed,
        "rendered": rendered,
    });
    app.state.set_validation(report);
    app.refresh_ui();
    ipc_runtime::emit_state(app);
    if let Some(id) = reply_to {
        let payload = IpcReplyPayload::ok(json!({ "report": report_value }));
        ipc_runtime::send_reply_payload(app, id, &payload);
    }
}
