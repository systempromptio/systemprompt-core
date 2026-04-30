use std::sync::Arc;

use crate::gui::GuiApp;
use crate::gui::error::GuiError;
use crate::gui::events::UiEvent;
use crate::{config, sync};

#[tracing::instrument(level = "debug", skip(app))]
pub(crate) fn on_sync_requested(app: &mut GuiApp) {
    if app.state.snapshot().sync_in_flight {
        return;
    }
    app.state.set_sync_in_flight(true);
    app.state.set_message("Sync started…");
    app.append_log("Sync started…");
    app.refresh_ui();
    app.pool.spawn_task(
        app.proxy.clone(),
        || {
            let allow_tofu = config::pinned_pubkey().is_none();
            sync::run_once(false, false, allow_tofu)
                .map_err(GuiError::from)
                .map_err(Arc::new)
        },
        UiEvent::SyncFinished,
    );
}

pub(crate) fn on_sync_started(app: &mut GuiApp) {
    app.state.set_sync_in_flight(true);
    app.refresh_ui();
}

pub(crate) fn on_sync_finished(
    app: &mut GuiApp,
    result: Result<crate::sync::SyncSummary, Arc<GuiError>>,
) {
    app.state.set_sync_in_flight(false);
    match result {
        Ok(summary) => {
            let line = summary.one_line();
            app.state.set_message(line.clone());
            app.append_log(&line);
        },
        Err(msg) => {
            let line = format!("sync failed: {msg}");
            app.state.set_message(line.clone());
            app.append_log(&line);
        },
    }
    app.state.reload();
    app.refresh_ui();
}
