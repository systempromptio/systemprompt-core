pub mod events;
pub mod state;
pub mod tray;
pub mod web;
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
use crate::gui::state::AppState;
use crate::gui::web::WebWindow;
use crate::http::GatewayClient;
use crate::output::diag;
use crate::{config, paths, setup, sync, validate};

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
    proxy: EventLoopProxy<UiEvent>,
    tray: Option<tray::TrayHandles>,
    window: WebWindow,
}

impl GuiApp {
    fn new(
        state: Arc<AppState>,
        tx: Sender<UiEvent>,
        proxy: EventLoopProxy<UiEvent>,
    ) -> Self {
        let window = WebWindow::new(tx);
        Self {
            state,
            proxy,
            tray: None,
            window,
        }
    }

    fn refresh_ui(&mut self) {
        let snap = self.state.snapshot();
        if let Some(handles) = &self.tray {
            tray::refresh(handles, &snap);
        }
        self.window.refresh(&snap);
    }

    fn dispatch(&mut self, event_loop: &ActiveEventLoop, event: UiEvent) {
        match event {
            UiEvent::OpenSettings => {
                let snap = self.state.snapshot();
                self.window.show(event_loop, &snap);
            },
            UiEvent::SyncRequested => {
                if self.state.snapshot().sync_in_flight {
                    return;
                }
                self.state.set_sync_in_flight(true);
                self.state.set_message("Sync started…");
                self.window.log("Sync started…");
                self.refresh_ui();
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = sync::run_once(false, false).map_err(|e| e.to_string());
                    let _ = proxy.send_event(UiEvent::SyncFinished(result));
                });
            },
            UiEvent::ValidateRequested => {
                self.window.log("Running validation…");
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
                    self.window.log("Login: PAT is empty");
                    self.refresh_ui();
                    return;
                }
                self.window.log("Saving PAT…");
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = setup::login(&token, gateway.as_deref())
                        .map(|_| ())
                        .map_err(|e| e.to_string());
                    let _ = proxy.send_event(UiEvent::LoginFinished(result));
                });
            },
            UiEvent::LogoutRequested => {
                self.window.log("Logging out…");
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
                        self.window.log(&line);
                    },
                    Err(msg) => {
                        let line = format!("sync failed: {msg}");
                        self.state.set_message(line.clone());
                        self.window.log(&line);
                    },
                }
                self.state.reload();
                self.refresh_ui();
            },
            UiEvent::ValidateFinished(report) => {
                let rendered = report.rendered();
                self.window.log(&rendered);
                self.state.set_validation(report);
                self.refresh_ui();
            },
            UiEvent::LoginFinished(result) => {
                match result {
                    Ok(()) => {
                        self.window.log("PAT stored. Pulling manifest…");
                        self.state.set_message("PAT stored.");
                        self.fetch_whoami_async();
                        self.state.reload();
                        self.refresh_ui();
                        self.dispatch(event_loop, UiEvent::SyncRequested);
                        return;
                    },
                    Err(e) => {
                        let line = format!("login failed: {e}");
                        self.window.log(&line);
                        self.state.set_message(line);
                    },
                }
                self.state.reload();
                self.refresh_ui();
            },
            UiEvent::LogoutFinished(result) => {
                match result {
                    Ok(()) => {
                        self.window.log("Logged out.");
                        self.state.set_message("Logged out.");
                    },
                    Err(e) => {
                        let line = format!("logout failed: {e}");
                        self.window.log(&line);
                        self.state.set_message(line);
                    },
                }
                self.state.reload();
                self.refresh_ui();
            },
            UiEvent::StateRefreshed => {
                self.state.reload();
                self.refresh_ui();
            },
        }
    }

    fn fetch_whoami_async(&self) {
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let cfg = config::load();
            let gateway = config::gateway_url_or_default(&cfg);
            let bearer = match crate::cache::read_valid() {
                Some(out) => out.token,
                None => {
                    use crate::providers::{AuthError, AuthProvider};
                    let chain: Vec<Box<dyn AuthProvider>> = vec![
                        Box::new(crate::providers::mtls::MtlsProvider::new(&cfg)),
                        Box::new(crate::providers::session::SessionProvider::new(&cfg)),
                        Box::new(crate::providers::pat::PatProvider::new(&cfg)),
                    ];
                    let mut token = None;
                    for p in &chain {
                        match p.authenticate() {
                            Ok(out) => {
                                let _ = crate::cache::write(&out);
                                token = Some(out.token);
                                break;
                            },
                            Err(AuthError::NotConfigured) => continue,
                            Err(AuthError::Failed(_)) => {},
                        }
                    }
                    match token {
                        Some(t) => t,
                        None => return,
                    }
                },
            };
            let client = GatewayClient::new(gateway);
            if let Ok(_value) = client.fetch_whoami(&bearer) {
                let _ = proxy.send_event(UiEvent::StateRefreshed);
            }
        });
    }
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
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UiEvent) {
        self.dispatch(event_loop, event);
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.owns(id) {
            self.window.handle_window_event(&event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(handles) = &self.tray {
            let drained = tray::drain(handles);
            for ev in drained {
                self.dispatch(event_loop, ev);
            }
        }
        event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_millis(250)));
    }
}
