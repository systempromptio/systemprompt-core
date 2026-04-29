use crate::config;
use crate::gui::GuiApp;
use crate::gui::events::UiEvent;
use crate::gui::state::{GatewayProbeOutcome, GatewayStatus, decode_jwt_identity, now_unix};
use crate::http::GatewayClient;

pub(crate) fn on_gateway_probe_requested(app: &mut GuiApp) {
    app.state.mark_probing();
    app.refresh_ui();
    spawn_probe(app);
}

pub(crate) fn on_gateway_probe_finished(app: &mut GuiApp, outcome: GatewayProbeOutcome) {
    app.state.apply_probe(outcome);
    app.refresh_ui();
}

pub(crate) fn spawn_probe(app: &GuiApp) {
    app.pool.spawn_task(
        app.proxy.clone(),
        || {
            let cfg = config::load();
            let gateway = config::gateway_url_or_default(&cfg);
            let client = GatewayClient::new(gateway);

            let started = std::time::Instant::now();
            let status = match client.health() {
                Ok(()) => GatewayStatus::Reachable {
                    latency_ms: started.elapsed().as_millis() as u64,
                },
                Err(e) => GatewayStatus::Unreachable {
                    reason: e.to_string(),
                },
            };

            let identity = if matches!(status, GatewayStatus::Reachable { .. })
                && crate::auth::has_credential_source(&cfg)
            {
                obtain_live_token(&cfg).and_then(|tok| decode_jwt_identity(tok.expose()))
            } else {
                if !crate::auth::has_credential_source(&cfg) {
                    let _ = crate::cache::clear();
                }
                None
            };

            GatewayProbeOutcome {
                status,
                identity,
                at_unix: now_unix(),
            }
        },
        UiEvent::GatewayProbeFinished,
    );
}

fn obtain_live_token(cfg: &config::Config) -> Option<crate::secret::Secret> {
    crate::auth::obtain_live_token(cfg).map(|out| out.token)
}
