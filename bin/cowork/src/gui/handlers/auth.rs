use crate::gui::GuiApp;
use crate::gui::events::UiEvent;
use crate::secret::Secret;
use crate::setup;

pub(crate) fn on_login_requested(app: &mut GuiApp, token: Secret, gateway: Option<String>) {
    let trimmed = Secret::new(token.expose().trim().to_owned());
    if trimmed.is_empty() {
        app.state.set_message("Login: PAT is empty");
        app.append_log("Login: PAT is empty");
        app.refresh_ui();
        return;
    }
    app.append_log("Saving PAT…");
    app.pool.spawn_with_proxy(app.proxy.clone(), move |proxy| {
        let result = setup::login(trimmed.expose(), gateway.as_deref())
            .map(|_| ())
            .map_err(|e| e.to_string());
        let _ = proxy.send_event(UiEvent::LoginFinished(result));
    });
}

pub(crate) fn on_login_finished(app: &mut GuiApp, result: Result<(), String>) {
    match result {
        Ok(()) => {
            app.append_log("PAT stored. Pulling manifest…");
            app.state.set_message("PAT stored.");
            super::gateway_probe::spawn_probe(app);
            app.state.reload();
            app.refresh_ui();
            let _ = app.proxy.send_event(UiEvent::SyncRequested);
            return;
        },
        Err(e) => {
            let line = format!("login failed: {e}");
            app.append_log(&line);
            app.state.set_message(line);
        },
    }
    app.state.reload();
    app.refresh_ui();
}

pub(crate) fn on_logout_requested(app: &mut GuiApp) {
    app.append_log("Logging out…");
    app.pool.spawn_with_proxy(app.proxy.clone(), |proxy| {
        let result = setup::logout().map(|_| ()).map_err(|e| e.to_string());
        let _ = proxy.send_event(UiEvent::LogoutFinished(result));
    });
}

pub(crate) fn on_logout_finished(app: &mut GuiApp, result: Result<(), String>) {
    match result {
        Ok(()) => {
            app.append_log("Logged out.");
            app.state.set_message("Logged out.");
        },
        Err(e) => {
            let line = format!("logout failed: {e}");
            app.append_log(&line);
            app.state.set_message(line);
        },
    }
    app.state.reload();
    app.refresh_ui();
}
