use systemprompt_extension::ExtensionContext;
use systemprompt_notifications_entry::http::NotificationsState;
use systemprompt_runtime::AppContext;

pub(crate) fn state(ctx: &AppContext) -> NotificationsState {
    NotificationsState::from_extension_state(&ctx.extension_state())
        .expect("the fixture context binds the notification service")
}
