use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::GuiApp;
use crate::gui::error::{GuiError, GuiResult};
use crate::gui::events::UiEvent;
use crate::gui::hosts::events::HostUiEvent;
use crate::integration::{
    GeneratedProfile, HostAppSnapshot, ProfileGenInputs, ProxyHealth, find_host_by_id, proxy_probe,
};

pub(crate) fn on_probe_requested(app: &mut GuiApp, host_id: &str) {
    let Some(host) = find_host_by_id(host_id) else {
        app.append_log(format!("probe requested for unknown host '{host_id}'"));
        return;
    };
    if !app.state.mark_host_probing(host_id) {
        return;
    }
    let host_id_owned = host_id.to_string();
    app.pool.spawn_task(
        app.proxy.clone(),
        move || Box::new(host.probe()),
        move |snap| {
            UiEvent::Host(HostUiEvent::ProbeFinished {
                host_id: host_id_owned.clone(),
                snapshot: snap,
            })
        },
    );
}

pub(crate) fn on_probe_finished(app: &mut GuiApp, host_id: &str, snapshot: HostAppSnapshot) {
    app.state.apply_host_snapshot(host_id, snapshot);
    let _ = app
        .proxy
        .send_event(UiEvent::Host(HostUiEvent::ProxyProbeRequested));
    app.refresh_ui();
}

pub(crate) fn on_proxy_probe_requested(app: &mut GuiApp) {
    let url = app.state.first_configured_proxy_url();
    if !app.state.mark_proxy_probing() {
        return;
    }
    app.pool.spawn_task(
        app.proxy.clone(),
        move || Box::new(proxy_probe::probe(url.as_deref())),
        |health| UiEvent::Host(HostUiEvent::ProxyProbeFinished(health)),
    );
}

pub(crate) fn on_proxy_probe_finished(app: &mut GuiApp, health: ProxyHealth) {
    app.state.apply_proxy_health(health);
    app.refresh_ui();
}

pub(crate) fn on_profile_generate_requested(app: &mut GuiApp, host_id: &str) {
    let Some(host) = find_host_by_id(host_id) else {
        app.append_log(format!("generate requested for unknown host '{host_id}'"));
        return;
    };
    app.append_log(format!("Generating profile for {}…", host.display_name()));
    let host_id_owned = host_id.to_string();
    app.pool.spawn_task(
        app.proxy.clone(),
        move || generate_profile_for(host),
        move |result| {
            UiEvent::Host(HostUiEvent::ProfileGenerateFinished {
                host_id: host_id_owned.clone(),
                result,
            })
        },
    );
}

pub(crate) fn on_profile_generate_finished(
    app: &mut GuiApp,
    host_id: &str,
    result: Result<GeneratedProfile, GuiError>,
) {
    match result {
        Ok(p) => {
            app.state
                .set_last_generated_profile(host_id, p.path.clone());
            app.append_log(format!(
                "[{host_id}] profile written: {} ({} bytes)",
                p.path, p.bytes
            ));
        },
        Err(e) => {
            app.append_log(format!("[{host_id}] profile generation failed: {e}"));
        },
    }
    app.refresh_ui();
}

pub(crate) fn on_profile_install_requested(app: &mut GuiApp, host_id: &str, path: String) {
    let Some(host) = find_host_by_id(host_id) else {
        app.append_log(format!("install requested for unknown host '{host_id}'"));
        return;
    };
    app.append_log(format!("[{host_id}] installing {path}…"));
    let host_id_owned = host_id.to_string();
    let path_clone = path.clone();
    app.pool.spawn_task(
        app.proxy.clone(),
        move || {
            host.install_profile(&path)
                .map(|_| path_clone.clone())
                .map_err(|e| GuiError::Profile(e.to_string()))
        },
        move |result| {
            UiEvent::Host(HostUiEvent::ProfileInstallFinished {
                host_id: host_id_owned.clone(),
                result,
            })
        },
    );
}

pub(crate) fn on_profile_install_finished(
    app: &mut GuiApp,
    host_id: &str,
    result: Result<String, GuiError>,
) {
    let action = find_host_by_id(host_id)
        .map(|h| h.install_action_label())
        .unwrap_or("installed");
    match result {
        Ok(path) => app.append_log(format!("[{host_id}] {action}: {path}")),
        Err(e) => app.append_log(format!("[{host_id}] profile install failed: {e}")),
    }
    let _ = app
        .proxy
        .send_event(UiEvent::Host(HostUiEvent::ProbeRequested {
            host_id: host_id.to_string(),
        }));
}

fn generate_profile_for(
    host: &'static dyn crate::integration::HostApp,
) -> GuiResult<GeneratedProfile> {
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

    let models = if server_profile.models.is_empty() {
        crate::integration::claude_desktop::default_models()
    } else {
        server_profile.models
    };

    let inputs = ProfileGenInputs {
        gateway_base_url: format!("http://localhost:{port}"),
        api_key: loopback_secret,
        models,
        organization_uuid: server_profile.organization_uuid,
    };
    host.generate_profile(&inputs)
        .map_err(|e| GuiError::Profile(e.to_string()))
}
