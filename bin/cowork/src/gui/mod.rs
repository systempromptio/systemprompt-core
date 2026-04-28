pub mod events;
pub mod server;
pub mod state;
pub mod tray;
pub mod window;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::gui::events::UiEvent;
use crate::gui::server::{ActivityLog, Server};
use crate::gui::state::{
    AppState, GatewayProbeOutcome, GatewayStatus, decode_jwt_identity, now_unix,
};
use crate::http::GatewayClient;
use crate::output::diag;
use crate::{config, paths, setup, sync, validate};

const PROBE_INTERVAL_SECS: u64 = 30;

pub fn run() -> ExitCode {
    let event_loop = match EventLoop::<UiEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            diag(&format!("gui: failed to build event loop: {e}"));
            return ExitCode::from(1);
        },
    };
    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = event_loop.create_proxy();
    let (tx, rx) = channel::<UiEvent>();

    let bridge_proxy = proxy.clone();
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            if bridge_proxy.send_event(event).is_err() {
                break;
            }
        }
    });

    let app_state = AppState::new_loaded();
    let mut app = GuiApp::new(app_state, tx, proxy);

    if let Err(e) = event_loop.run_app(&mut app) {
        diag(&format!("gui: event loop error: {e}"));
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

struct GuiApp {
    state: Arc<AppState>,
    tx: Sender<UiEvent>,
    proxy: EventLoopProxy<UiEvent>,
    tray: Option<tray::TrayHandles>,
    server: Option<Server>,
}

impl GuiApp {
    fn new(
        state: Arc<AppState>,
        tx: Sender<UiEvent>,
        proxy: EventLoopProxy<UiEvent>,
    ) -> Self {
        Self {
            state,
            tx,
            proxy,
            tray: None,
            server: None,
        }
    }

    fn refresh_ui(&mut self) {
        let snap = self.state.snapshot();
        if let Some(handles) = &self.tray {
            tray::refresh(handles, &snap);
        }
    }

    fn ensure_server(&mut self) -> Option<&Server> {
        if self.server.is_none() {
            match Server::start(self.state.clone(), self.tx.clone()) {
                Ok(s) => {
                    s.log()
                        .append(format!("settings ui served at {}", s.url()));
                    self.server = Some(s);
                },
                Err(e) => {
                    diag(&format!("gui: failed to start settings server: {e}"));
                    return None;
                },
            }
        }
        self.server.as_ref()
    }

    fn log(&self) -> Option<&ActivityLog> {
        self.server.as_ref().map(|s| s.log())
    }

    fn append_log(&self, line: impl Into<String>) {
        if let Some(log) = self.log() {
            log.append(line);
        }
    }

    fn dispatch(&mut self, _event_loop: &ActiveEventLoop, event: UiEvent) {
        match event {
            UiEvent::OpenSettings => {
                if let Some(server) = self.ensure_server() {
                    let url = server.url();
                    self.append_log(format!("opening {url}"));
                    window::open_url(&url);
                }
            },
            UiEvent::SyncRequested => {
                if self.state.snapshot().sync_in_flight {
                    return;
                }
                self.state.set_sync_in_flight(true);
                self.state.set_message("Sync started…");
                self.append_log("Sync started…");
                self.refresh_ui();
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = sync::run_once(false, false, false).map_err(|e| e.to_string());
                    let _ = proxy.send_event(UiEvent::SyncFinished(result));
                });
            },
            UiEvent::ValidateRequested => {
                self.append_log("Running validation…");
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let report = validate::run();
                    let _ = proxy.send_event(UiEvent::ValidateFinished(report));
                });
            },
            UiEvent::OpenConfigFolder => {
                if let Some(loc) = paths::org_plugins_effective() {
                    window::open_path(&loc.path);
                } else if let Ok(s) = setup::status() {
                    window::open_path(&s.paths.config_dir);
                }
            },
            UiEvent::LoginRequested { token, gateway } => {
                let token = token.trim().to_string();
                if token.is_empty() {
                    self.state.set_message("Login: PAT is empty");
                    self.append_log("Login: PAT is empty");
                    self.refresh_ui();
                    return;
                }
                self.append_log("Saving PAT…");
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = setup::login(&token, gateway.as_deref())
                        .map(|_| ())
                        .map_err(|e| e.to_string());
                    let _ = proxy.send_event(UiEvent::LoginFinished(result));
                });
            },
            UiEvent::LogoutRequested => {
                self.append_log("Logging out…");
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = setup::logout().map(|_| ()).map_err(|e| e.to_string());
                    let _ = proxy.send_event(UiEvent::LogoutFinished(result));
                });
            },
            UiEvent::Quit => {
                std::process::exit(0);
            },

            UiEvent::SyncStarted => {
                self.state.set_sync_in_flight(true);
                self.refresh_ui();
            },
            UiEvent::SyncFinished(result) => {
                self.state.set_sync_in_flight(false);
                match result {
                    Ok(summary) => {
                        let line = summary.one_line();
                        self.state.set_message(line.clone());
                        self.append_log(&line);
                    },
                    Err(msg) => {
                        let line = format!("sync failed: {msg}");
                        self.state.set_message(line.clone());
                        self.append_log(&line);
                    },
                }
                self.state.reload();
                self.refresh_ui();
            },
            UiEvent::ValidateFinished(report) => {
                let rendered = report.rendered();
                self.append_log(rendered);
                self.state.set_validation(report);
                self.refresh_ui();
            },
            UiEvent::LoginFinished(result) => {
                match result {
                    Ok(()) => {
                        self.append_log("PAT stored. Pulling manifest…");
                        self.state.set_message("PAT stored.");
                        self.probe_gateway_async();
                        self.state.reload();
                        self.refresh_ui();
                        let _ = self.proxy.send_event(UiEvent::SyncRequested);
                        return;
                    },
                    Err(e) => {
                        let line = format!("login failed: {e}");
                        self.append_log(&line);
                        self.state.set_message(line);
                    },
                }
                self.state.reload();
                self.refresh_ui();
            },
            UiEvent::LogoutFinished(result) => {
                match result {
                    Ok(()) => {
                        self.append_log("Logged out.");
                        self.state.set_message("Logged out.");
                    },
                    Err(e) => {
                        let line = format!("logout failed: {e}");
                        self.append_log(&line);
                        self.state.set_message(line);
                    },
                }
                self.state.reload();
                self.refresh_ui();
            },
            UiEvent::GatewayProbeRequested => {
                self.state.mark_probing();
                self.refresh_ui();
                self.probe_gateway_async();
            },
            UiEvent::GatewayProbeFinished(outcome) => {
                self.state.apply_probe(outcome);
                self.refresh_ui();
            },
            UiEvent::StateRefreshed => {
                self.state.reload();
                self.refresh_ui();
            },

            #[cfg(target_os = "macos")]
            UiEvent::ClaudeProbeRequested => {
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let snap = crate::integration::claude_desktop::probe();
                    let _ = proxy.send_event(UiEvent::ClaudeProbeFinished(snap));
                });
            },
            #[cfg(target_os = "macos")]
            UiEvent::ClaudeProbeFinished(snap) => {
                self.state.apply_claude_integration(snap);
                self.refresh_ui();
            },
            #[cfg(target_os = "macos")]
            UiEvent::ClaudeProfileGenerateRequested => {
                self.append_log("Generating Claude Desktop profile…");
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = generate_claude_profile().map_err(|e| e.to_string());
                    let _ = proxy.send_event(UiEvent::ClaudeProfileGenerateFinished(result));
                });
            },
            #[cfg(target_os = "macos")]
            UiEvent::ClaudeProfileGenerateFinished(result) => {
                match result {
                    Ok(p) => {
                        self.state.set_last_generated_profile(p.path.clone());
                        self.append_log(format!(
                            "profile written: {} ({} bytes)",
                            p.path, p.bytes
                        ));
                    },
                    Err(e) => {
                        self.append_log(format!("profile generation failed: {e}"));
                    },
                }
                self.refresh_ui();
            },
            #[cfg(target_os = "macos")]
            UiEvent::ClaudeProfileInstallRequested(path) => {
                self.append_log(format!("opening {path} in System Settings…"));
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = crate::integration::claude_desktop::install_profile(&path)
                        .map(|_| path.clone())
                        .map_err(|e| e.to_string());
                    let _ = proxy.send_event(UiEvent::ClaudeProfileInstallFinished(result));
                });
            },
            #[cfg(target_os = "macos")]
            UiEvent::ClaudeProfileInstallFinished(result) => {
                match result {
                    Ok(path) => self
                        .append_log(format!("profile handed to System Settings: {path}")),
                    Err(e) => self.append_log(format!("profile install failed: {e}")),
                }
                let _ = self.proxy.send_event(UiEvent::ClaudeProbeRequested);
            },
        }
    }

    fn probe_gateway_async(&self) {
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
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

            let identity = if matches!(status, GatewayStatus::Reachable { .. }) {
                obtain_live_token(&cfg).and_then(|tok| decode_jwt_identity(&tok))
            } else {
                None
            };

            let _ = proxy.send_event(UiEvent::GatewayProbeFinished(GatewayProbeOutcome {
                status,
                identity,
                at_unix: now_unix(),
            }));
        });
    }
}

#[cfg(target_os = "macos")]
fn generate_claude_profile()
-> Result<crate::integration::claude_desktop::GeneratedProfile, String> {
    use crate::integration::claude_desktop::{ProfileGenInputs, write_profile};

    let cfg = config::load();
    let token = obtain_live_token(&cfg)
        .ok_or_else(|| "no live JWT available — sign in first".to_string())?;

    let inputs = ProfileGenInputs {
        gateway_base_url: config::claude_inference_base_url(&cfg),
        api_key: token,
        models: config::claude_models(&cfg),
        organization_uuid: config::claude_organization_uuid(&cfg),
    };
    write_profile(&inputs).map_err(|e| e.to_string())
}

fn obtain_live_token(cfg: &config::Config) -> Option<String> {
    if let Some(out) = crate::cache::read_valid() {
        return Some(out.token);
    }
    use crate::providers::{AuthError, AuthProvider};
    let chain: Vec<Box<dyn AuthProvider>> = vec![
        Box::new(crate::providers::mtls::MtlsProvider::new(cfg)),
        Box::new(crate::providers::session::SessionProvider::new(cfg)),
        Box::new(crate::providers::pat::PatProvider::new(cfg)),
    ];
    for p in &chain {
        match p.authenticate() {
            Ok(out) => {
                let _ = crate::cache::write(&out);
                return Some(out.token);
            },
            Err(AuthError::NotConfigured) => continue,
            Err(AuthError::Failed(_)) => {},
        }
    }
    None
}

impl ApplicationHandler<UiEvent> for GuiApp {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) {
            return;
        }
        if let Some(handles) = &self.tray {
            for ev in tray::drain(handles) {
                self.dispatch(event_loop, ev);
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.tray.is_none() {
            let snap = self.state.snapshot();
            match tray::build(&snap) {
                Ok(handles) => self.tray = Some(handles),
                Err(e) => diag(&format!("gui: tray init failed: {e}")),
            }
        }
        self.refresh_ui();
        self.dispatch(event_loop, UiEvent::OpenSettings);
        let _ = self.proxy.send_event(UiEvent::GatewayProbeRequested);
        #[cfg(target_os = "macos")]
        {
            let _ = self.proxy.send_event(UiEvent::ClaudeProbeRequested);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UiEvent) {
        self.dispatch(event_loop, event);
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _id: WindowId,
        _event: WindowEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(handles) = &self.tray {
            for ev in tray::drain(handles) {
                let _ = self.proxy.send_event(ev);
            }
        }
        let snap = self.state.snapshot();
        let needs_probe = matches!(snap.gateway_status, GatewayStatus::Unknown)
            || snap
                .last_probe_at_unix
                .map(|t| now_unix().saturating_sub(t) >= PROBE_INTERVAL_SECS)
                .unwrap_or(true);
        if needs_probe && !matches!(snap.gateway_status, GatewayStatus::Probing) {
            let _ = self.proxy.send_event(UiEvent::GatewayProbeRequested);
        }
        #[cfg(target_os = "macos")]
        {
            let claude_due = snap
                .claude_integration
                .as_ref()
                .map(|c| now_unix().saturating_sub(c.probed_at_unix) >= PROBE_INTERVAL_SECS)
                .unwrap_or(true);
            if claude_due {
                let _ = self.proxy.send_event(UiEvent::ClaudeProbeRequested);
            }
        }
        event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_secs(1)));
    }
}
