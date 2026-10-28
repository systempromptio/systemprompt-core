//! Projecting the Drive status onto the GUI: the `drive.changed` event, the
//! tray's Drive line and the conflict, editor, sharing, proposal and
//! sent-for-review toasts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::drive::DriveStatus;
use crate::drive::status::QueuedNotice;
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
    raise_notices(app, &status);
    raise_reviews(app, &status);
}

/// One toast per local edit the server took as a proposal since the last
/// tick: the edit is waiting for a reviewer and the local copy is kept.
fn raise_reviews(app: &mut GuiApp, status: &DriveStatus) {
    let since = app.drive_review_since;
    for sent in status
        .sent_for_review
        .iter()
        .filter(|sent| sent.sent_at > since)
    {
        crate::gui::window::notify_user(
            &i18n::t("drive-review-title"),
            &i18n::t_args("drive-review-body", &[("path", &sent.path)]),
        );
        app.drive_review_since = app.drive_review_since.max(sent.sent_at);
    }
}

fn raise_notices(app: &mut GuiApp, status: &DriveStatus) {
    for queued in status
        .notices
        .iter()
        .filter(|queued| queued.seq > app.drive_notice_seq)
    {
        let (title, body) = notice_text(status, queued);
        crate::gui::window::notify_user(&title, &body);
    }
    if let Some(last) = status.notices.last() {
        app.drive_notice_seq = app.drive_notice_seq.max(last.seq);
    }
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

fn notice_text(status: &DriveStatus, queued: &QueuedNotice) -> (String, String) {
    let text = crate::drive::notice::notice_text(status, &queued.notice);
    let args: Vec<(&str, &str)> = text
        .args
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    (i18n::t(text.title_key), i18n::t_args(text.body_key, &args))
}
