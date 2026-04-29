use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::GuiApp;
use crate::gui::claude::events::ClaudeUiEvent;
use crate::gui::error::{GuiError, GuiResult};
use crate::gui::events::UiEvent;
use crate::integration::claude_desktop::{
    ClaudeIntegrationSnapshot, GeneratedProfile, ProfileGenInputs, write_profile,
};

pub(crate) fn on_probe_requested(app: &mut GuiApp) {
    if !app.state.mark_claude_probing() {
        return;
    }
    app.pool.spawn_task(
        app.proxy.clone(),
        || Box::new(crate::integration::claude_desktop::probe()),
        |snap| UiEvent::Claude(ClaudeUiEvent::ProbeFinished(snap)),
    );
}

pub(crate) fn on_probe_finished(app: &mut GuiApp, snap: Box<ClaudeIntegrationSnapshot>) {
    app.state.apply_claude_integration(*snap);
    app.refresh_ui();
}

pub(crate) fn on_profile_generate_requested(app: &mut GuiApp) {
    app.append_log("Generating Claude Desktop profile…");
    app.pool
        .spawn_task(app.proxy.clone(), generate_claude_profile, |result| {
            UiEvent::Claude(ClaudeUiEvent::ProfileGenerateFinished(result))
        });
}

pub(crate) fn on_profile_generate_finished(
    app: &mut GuiApp,
    result: Result<GeneratedProfile, GuiError>,
) {
    match result {
        Ok(p) => {
            app.state.set_last_generated_profile(p.path.clone());
            app.append_log(format!("profile written: {} ({} bytes)", p.path, p.bytes));
        },
        Err(e) => {
            app.append_log(format!("profile generation failed: {e}"));
        },
    }
    app.refresh_ui();
}

pub(crate) fn on_profile_install_requested(app: &mut GuiApp, path: String) {
    app.append_log(format!("opening {path} in System Settings…"));
    app.pool.spawn_task(
        app.proxy.clone(),
        move || {
            crate::integration::claude_desktop::install_profile(&path)
                .map(|_| path.clone())
                .map_err(|e| GuiError::Profile(e.to_string()))
        },
        |result| UiEvent::Claude(ClaudeUiEvent::ProfileInstallFinished(result)),
    );
}

pub(crate) fn on_profile_install_finished(app: &mut GuiApp, result: Result<String, GuiError>) {
    match result {
        Ok(path) => app.append_log(format!("profile handed to System Settings: {path}")),
        Err(e) => app.append_log(format!("profile install failed: {e}")),
    }
    let _ = app
        .proxy
        .send_event(UiEvent::Claude(ClaudeUiEvent::ProbeRequested));
}

fn generate_claude_profile() -> GuiResult<GeneratedProfile> {
    let cfg = config::load();

    let port = crate::proxy::handle()
        .map(|h| h.port)
        .unwrap_or(crate::proxy::DEFAULT_PROXY_PORT);

    let loopback_secret = crate::proxy::secret::load_or_mint()
        .map_err(|e| GuiError::Profile(format!("loopback secret: {e}")))?;

    let gateway_base = config::gateway_url_or_default(&cfg);
    let server_profile = GatewayClient::new(gateway_base)
        .fetch_cowork_profile()
        .map_err(|e| GuiError::Profile(format!("fetch /v1/cowork/profile failed: {e}")))?;

    let inputs = ProfileGenInputs {
        gateway_base_url: format!("http://localhost:{port}"),
        api_key: loopback_secret,
        models: server_profile.models,
        organization_uuid: server_profile.organization_uuid,
    };
    write_profile(&inputs).map_err(|e| GuiError::Profile(e.to_string()))
}
