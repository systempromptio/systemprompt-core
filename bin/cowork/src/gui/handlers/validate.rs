use crate::gui::GuiApp;
use crate::gui::events::UiEvent;
use crate::validate;

pub(crate) fn on_validate_requested(app: &mut GuiApp) {
    app.append_log("Running validation…");
    app.pool
        .spawn_task(app.proxy.clone(), validate::run, UiEvent::ValidateFinished);
}

pub(crate) fn on_validate_finished(app: &mut GuiApp, report: validate::ValidationReport) {
    let rendered = report.rendered();
    app.append_log(rendered);
    app.state.set_validation(report);
    app.refresh_ui();
}
