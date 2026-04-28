use crate::config;
use crate::gui::GuiApp;
use crate::gui::events::UiEvent;
use crate::http::GatewayClient;
use crate::integration::claude_desktop::{
    ClaudeIntegrationSnapshot, GeneratedProfile, ProfileGenInputs, write_profile,
};

pub(crate) fn on_claude_probe_requested(app: &mut GuiApp) {
    if !app.state.mark_claude_probing() {
        return;
    }
    app.pool.spawn_with_proxy(app.proxy.clone(), |proxy| {
        let snap = crate::integration::claude_desktop::probe();
        let _ = proxy.send_event(UiEvent::ClaudeProbeFinished(snap));
    });
}

pub(crate) fn on_claude_probe_finished(app: &mut GuiApp, snap: ClaudeIntegrationSnapshot) {
    app.state.apply_claude_integration(snap);
    app.refresh_ui();
}

pub(crate) fn on_claude_profile_generate_requested(app: &mut GuiApp) {
    app.append_log("Generating Claude Desktop profile…");
    app.pool.spawn_with_proxy(app.proxy.clone(), |proxy| {
        let result = generate_claude_profile().map_err(|e| e.to_string());
        let _ = proxy.send_event(UiEvent::ClaudeProfileGenerateFinished(result));
    });
}

pub(crate) fn on_claude_profile_generate_finished(
    app: &mut GuiApp,
    result: Result<GeneratedProfile, String>,
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

pub(crate) fn on_claude_profile_install_requested(app: &mut GuiApp, path: String) {
    app.append_log(format!("opening {path} in System Settings…"));
    app.pool.spawn_with_proxy(app.proxy.clone(), move |proxy| {
        let result = crate::integration::claude_desktop::install_profile(&path)
            .map(|_| path.clone())
            .map_err(|e| e.to_string());
        let _ = proxy.send_event(UiEvent::ClaudeProfileInstallFinished(result));
    });
}

pub(crate) fn on_claude_profile_install_finished(app: &mut GuiApp, result: Result<String, String>) {
    match result {
        Ok(path) => app.append_log(format!("profile handed to System Settings: {path}")),
        Err(e) => app.append_log(format!("profile install failed: {e}")),
    }
    let _ = app.proxy.send_event(UiEvent::ClaudeProbeRequested);
}

fn generate_claude_profile() -> Result<GeneratedProfile, String> {
    let cfg = config::load();

    let port = crate::proxy::handle()
        .map(|h| h.port)
        .unwrap_or(crate::proxy::DEFAULT_PROXY_PORT);

    let loopback_secret =
        crate::proxy::secret::load_or_mint().map_err(|e| format!("loopback secret: {e}"))?;

    let gateway_base = config::gateway_url_or_default(&cfg);
    let server_profile = GatewayClient::new(gateway_base)
        .fetch_cowork_profile()
        .map_err(|e| format!("fetch /v1/cowork/profile failed: {e}"))?;

    let inputs = ProfileGenInputs {
        gateway_base_url: format!("http://localhost:{port}"),
        api_key: loopback_secret,
        models: server_profile.models,
        organization_uuid: server_profile.organization_uuid,
    };
    write_profile(&inputs).map_err(|e| e.to_string())
}
