//! Projecting the Drive status onto the GUI: the `drive.changed` event, the
//! tray's Drive line and the conflict and editor toasts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::drive::DriveStatus;
use crate::gui::GuiApp;
use crate::gui::notify::Signal;
use crate::i18n;
use crate::wire::drive::{DriveCode, DriveStatusPayload};

pub(crate) fn tick(app: &mut GuiApp) {
    if !app.drive_watch.has_changed().unwrap_or(false) {
        return;
    }
    app.drive_watch.mark_unchanged();
    let status = app.ctx.drive.status();
    let payload = DriveStatusPayload::from(status.as_ref());
    crate::gui::emit::send_emit(app, "drive.changed", &payload);
    if let Some(handles) = &app.tray {
        handles.drive_item.set_text(tray_line(&status));
    }
    raise_toasts(app, &status);
}

#[must_use]
pub(crate) fn tray_line(status: &DriveStatus) -> String {
    let code = DriveCode::of(status);
    let line = i18n::t(&format!("tray-drive-{}", code_name(code)));
    let editing = status.editing_count();
    if editing == 0 {
        return line;
    }
    i18n::t_args(
        "tray-editing-count",
        &[("line", &line), ("count", &editing.to_string())],
    )
}

const fn code_name(code: DriveCode) -> &'static str {
    match code {
        DriveCode::Off => "off",
        DriveCode::Synced => "synced",
        DriveCode::Pending => "pending",
        DriveCode::Conflicts => "conflicts",
        DriveCode::Error => "error",
    }
}

fn raise_toasts(app: &mut GuiApp, status: &DriveStatus) {
    match status.conflicts.last() {
        Some(latest) => app.signal_raised(
            Signal::DriveConflict,
            &i18n::t("drive-conflict-title"),
            &i18n::t_args(
                "drive-conflict-body",
                &[("path", &latest.path), ("sibling", &latest.sibling)],
            ),
        ),
        None => app.signal_cleared(Signal::DriveConflict),
    }
    match status.editing_count() {
        0 => app.signal_cleared(Signal::DriveEditor),
        count => app.signal_raised(
            Signal::DriveEditor,
            &i18n::t("drive-editor-title"),
            &i18n::t_args("drive-editor-body", &[("count", &count.to_string())]),
        ),
    }
}
