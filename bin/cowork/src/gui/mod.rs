pub mod events;
pub mod state;
pub mod tray;
pub mod window;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use winit::application::ApplicationHandler;
use winit::event::StartCause;
use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::gui::events::UiEvent;
use crate::gui::state::AppState;
use crate::http::GatewayClient;
use crate::{config, paths, setup, sync, validate};

pub fn run() -> ExitCode {
    let event_loop = match EventLoop::<UiEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("gui: failed to build event loop: {e}");
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
    let mut app = match GuiApp::new(app_state, tx, proxy) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("gui: {e}");
            return ExitCode::from(1);
        },
    };

    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("gui: event loop error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

struct GuiApp {
    state: Arc<AppState>,
    tx: Sender<UiEvent>,
    proxy: EventLoopProxy<UiEvent>,
    tray: Option<tray::TrayHandles>,
    window: Option<window::PlatformWindow>,
}

impl GuiApp {
    fn new(
        state: Arc<AppState>,
        tx: Sender<UiEvent>,
        proxy: EventLoopProxy<UiEvent>,
    ) -> Result<Self, String> {
        Ok(Self {
            state,
            tx,
            proxy,
            tray: None,
            window: None,
        })
    }

    fn refresh_ui(&mut self) {
        let snap = self.state.snapshot();
        if let Some(handles) = &self.tray {
            tray::refresh(handles, &snap);
        }
        if let Some(window) = &self.window {
            window.refresh(&snap);
        }
    }

    fn dispatch(&mut self, event: UiEvent) {
        match event {
            UiEvent::OpenSettings => {
                if self.window.is_none() {
                    match window::PlatformWindow::new(self.tx.clone()) {
                        Ok(w) => self.window = Some(w),
                        Err(e) => self.state.set_message(format!("settings window: {e}")),
                    }
                }
                let snap = self.state.snapshot();
                if let Some(w) = &self.window {
                    w.show(&snap);
                }
            },
            UiEvent::SyncRequested => {
                if self.state.snapshot().sync_in_flight {
                    return;
                }
                self.state.set_sync_in_flight(true);
                self.state.set_message("Sync started…");
                self.refresh_ui();
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = sync::run_once(false).map_err(|e| e.message);
                    let _ = proxy.send_event(UiEvent::SyncFinished(result));
                });
            },
            UiEvent::ValidateRequested => {
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
                    self.state
                        .set_message("Login: PAT is empty");
                    self.refresh_ui();
                    return;
                }
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = setup::login(&token, gateway.as_deref()).map(|_| ());
                    let _ = proxy.send_event(UiEvent::LoginFinished(result));
                });
            },
            UiEvent::LogoutRequested => {
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = setup::logout().map(|_| ());
                    let _ = proxy.send_event(UiEvent::LogoutFinished(result));
                });
            },
            UiEvent::Quit => {
                self.proxy.send_event(UiEvent::StateRefreshed).ok();
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
                        self.state.set_message(summary.one_line());
                    },
                    Err(msg) => {
                        self.state.set_message(format!("sync failed: {msg}"));
                    },
                }
                self.state.reload();
                self.refresh_ui();
            },
            UiEvent::ValidateFinished(report) => {
                self.state.set_validation(report);
                self.refresh_ui();
            },
            UiEvent::LoginFinished(result) => {
                match result {
                    Ok(()) => {
                        self.state.set_message("Login: PAT stored.");
                        self.fetch_whoami_async();
                    },
                    Err(e) => self.state.set_message(format!("login failed: {e}")),
                }
                self.state.reload();
                self.refresh_ui();
            },
            UiEvent::LogoutFinished(result) => {
                match result {
                    Ok(()) => self.state.set_message("Logout: PAT removed."),
                    Err(e) => self.state.set_message(format!("logout failed: {e}")),
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
    fn new_events(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, cause: StartCause) {
        let _ = event_loop;
        if matches!(cause, StartCause::Init) {
            return;
        }
        if let Some(handles) = &self.tray {
            for ev in tray::drain(handles) {
                self.dispatch(ev);
            }
        }
    }

    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.tray.is_none() {
            let snap = self.state.snapshot();
            match tray::build(&snap) {
                Ok(handles) => self.tray = Some(handles),
                Err(e) => eprintln!("gui: tray init failed: {e}"),
            }
        }
        self.refresh_ui();
    }

    fn user_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        event: UiEvent,
    ) {
        self.dispatch(event);
    }

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _id: WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(handles) = &self.tray {
            let drained = tray::drain(handles);
            for ev in drained {
                self.dispatch(ev);
            }
        }
        event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_millis(250)));
    }
}
