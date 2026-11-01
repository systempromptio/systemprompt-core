//! Projecting the inbox onto the GUI: the `inbox.changed` event, the tray's
//! Inbox line, the request to show the pane and the toasts for notifications
//! that need the reader.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::Ordering;

use crate::gui::GuiApp;
use crate::i18n;
use crate::integration::host_app::session_starter;
use crate::proxy::notifications::InboxToast;
use crate::wire::external_url::ExternalUrl;
use crate::wire::inbox::{InboxPayload, site_url};

#[must_use]
pub(crate) fn payload(app: &GuiApp) -> InboxPayload {
    InboxPayload::new(
        app.ctx.inbox.snapshot(),
        app.ctx.inbox.available(),
        app.ctx.inbox.last_error(),
        session_starter().is_some(),
    )
}

pub(crate) fn tick(app: &mut GuiApp) {
    if !app.inbox_watch.has_changed().unwrap_or(false) {
        return;
    }
    app.inbox_watch.mark_unchanged();
    let payload = payload(app);
    crate::gui::emit::send_emit(app, "inbox.changed", &payload);
    if let Some(handles) = &app.tray {
        handles.inbox_item.set_text(tray_line(&payload));
    }
    for toast in app.ctx.inbox.take_toasts() {
        raise(app, toast);
    }
}

#[must_use]
pub(crate) fn tray_line(payload: &InboxPayload) -> String {
    if !payload.available {
        return i18n::t("nav-inbox");
    }
    let count = if payload.unread_capped {
        i18n::t_args(
            "inbox-count-capped",
            &[("count", &payload.unread.max(0).to_string())],
        )
    } else {
        payload.unread.max(0).to_string()
    };
    i18n::t_args("tray-inbox", &[("count", &count)])
}

// Why: a webview opened by this request has not loaded its script yet, so the
// show is held until its inbox pane first asks for the list.
pub(crate) fn show(app: &GuiApp, was_open: bool) {
    if was_open {
        crate::gui::emit::send_emit(app, "inbox.show", &serde_json::Value::Null);
    } else {
        app.inbox_show_pending.store(true, Ordering::Release);
    }
}

pub(crate) fn take_pending_show(app: &GuiApp) {
    if app.inbox_show_pending.swap(false, Ordering::AcqRel) {
        crate::gui::emit::send_emit(app, "inbox.show", &serde_json::Value::Null);
    }
}

#[must_use]
pub(crate) fn resolve_open_url(app: &GuiApp, open_url: &str) -> Option<ExternalUrl> {
    let base = app.state.snapshot().gateway_url;
    match site_url(&base, open_url) {
        Ok(url) => Some(url),
        Err(error) => {
            tracing::warn!(%error, "notification link not opened");
            None
        },
    }
}

fn raise(app: &GuiApp, toast: InboxToast) {
    match toast {
        InboxToast::Item(item) => {
            let url = item
                .open_url
                .as_deref()
                .and_then(|link| resolve_open_url(app, link));
            let body = if item.body.is_empty() {
                i18n::t("inbox-toast-body")
            } else {
                item.body
            };
            crate::gui::window::notify_user_with_action(&item.title, &body, url);
        },
        InboxToast::More { count } => crate::gui::window::notify_user(
            &i18n::t("inbox-toast-more-title"),
            &i18n::t_args("inbox-toast-more-body", &[("count", &count.to_string())]),
        ),
    }
}
