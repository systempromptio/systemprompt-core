use crate::auth::secret::Secret;
use crate::auth::setup;
use crate::gui::GuiApp;
use crate::gui::error::GuiError;
use crate::gui::events::UiEvent;

pub(crate) fn on_login_requested(app: &mut GuiApp, token: &Secret, gateway: Option<String>) {
    let trimmed = Secret::new(token.expose().trim().to_owned());
    if trimmed.is_empty() {
        app.state.set_message("Login: PAT is empty");
        app.append_log("Login: PAT is empty");
        app.refresh_ui();
        return;
    }
    app.append_log("Saving PAT…");
    app.pool.spawn_task(
        app.proxy.clone(),
        move || {
            setup::login(trimmed.expose(), gateway.as_deref())
                .map(|_| ())
                .map_err(GuiError::from)
        },
        UiEvent::LoginFinished,
    );
}

pub(crate) fn on_login_finished(app: &mut GuiApp, result: Result<(), GuiError>) {
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

pub(crate) fn on_set_gateway_requested(app: &mut GuiApp, gateway: &str) {
    let trimmed = gateway.trim().to_owned();
    if trimmed.is_empty() {
        app.state.set_message("Set gateway: URL is empty");
        app.append_log("Set gateway: URL is empty");
        app.refresh_ui();
        return;
    }
    app.append_log(format!("Saving gateway URL {trimmed}…"));
    app.pool.spawn_task(
        app.proxy.clone(),
        move || {
            setup::set_gateway_url(&trimmed)
                .map(|_| ())
                .map_err(GuiError::from)
        },
        UiEvent::SetGatewayFinished,
    );
}

pub(crate) fn on_set_gateway_finished(app: &mut GuiApp, result: Result<(), GuiError>) {
    match result {
        Ok(()) => {
            app.append_log("Gateway URL saved.");
            app.state.reload();
            super::gateway_probe::spawn_probe(app);
        },
        Err(e) => {
            let line = format!("set gateway failed: {e}");
            app.append_log(&line);
            app.state.set_message(line);
            app.state.reload();
        },
    }
    app.refresh_ui();
}

pub(crate) fn on_logout_requested(app: &mut GuiApp) {
    app.append_log("Logging out…");
    app.pool.spawn_task(
        app.proxy.clone(),
        || setup::logout().map(|_| ()).map_err(GuiError::from),
        UiEvent::LogoutFinished,
    );
}

pub(crate) fn on_logout_finished(app: &mut GuiApp, result: Result<(), GuiError>) {
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
