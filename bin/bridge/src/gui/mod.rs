pub mod agents_state;
pub mod assets;
pub mod command;
pub mod dispatch;
pub mod error;
pub mod events;
pub mod handlers;

pub mod hosts;
pub mod ipc;
pub mod ipc_runtime;
pub mod menu;
pub mod server;
pub mod server_json;
pub mod server_marketplace;
pub mod server_util;
pub mod state;
pub mod tray;
pub mod window;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::gui::events::UiEvent;
use crate::gui::server::Server;
use crate::gui::state::{AppState, GatewayStatus, now_unix};
use crate::gui::window::SettingsWindow;
use crate::obs::output::diag;
use tokio::runtime::Handle;

pub(crate) const PROBE_INTERVAL_SECS: u64 = 30;
const PROXY_STATS_TICK_SECS: u64 = 1;

fn install_termination_handlers(proxy: EventLoopProxy<UiEvent>) {
    let _ = ctrlc::set_handler(move || {
        let _ = proxy.send_event(UiEvent::Quit);
    });
}

#[tracing::instrument]
pub fn run() -> ExitCode {
    let proxy_started = crate::proxy::start_default().is_some();

    let event_loop = match EventLoop::<UiEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            diag(&format!("gui: failed to build event loop: {e}"));
            return ExitCode::from(1);
        },
    };
    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = event_loop.create_proxy();
    install_termination_handlers(proxy.clone());
    crate::gui::ipc_runtime::install_log_emitter(proxy.clone());
    let (tx, rx) = channel::<UiEvent>();

    let bridge_proxy = proxy.clone();
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            if bridge_proxy.send_event(event).is_err() {
                break;
            }
        }
    });

    let runtime = match crate::proxy::runtime_handle() {
        Ok(h) => h,
        Err(e) => {
            diag(&format!("gui: tokio runtime unavailable: {e}"));
            return ExitCode::from(1);
        },
    };
    let app_state = AppState::new_loaded();
    let mut app = GuiApp::new(app_state, tx, proxy, runtime);

    if proxy_started {
        if let Some(h) = crate::proxy::handle() {
            app.append_log(format!("local proxy listening on 127.0.0.1:{}", h.port));
        }
    } else {
        app.append_log(format!(
            "local proxy FAILED to start on port {} — host requests will be refused. \
             Another process may be bound to that port; check the log file for details.",
            crate::proxy::DEFAULT_PROXY_PORT
        ));
    }

    if let Err(e) = event_loop.run_app(&mut app) {
        diag(&format!("gui: event loop error: {e}"));
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

pub(crate) struct GuiApp {
    pub(crate) state: Arc<AppState>,
    pub(crate) tx: Sender<UiEvent>,
    pub(crate) proxy: EventLoopProxy<UiEvent>,
    pub(crate) tray: Option<tray::TrayHandles>,
    pub(crate) menu_bar: Option<menu::MenuBarHandles>,
    pub(crate) server: Option<Server>,
    pub(crate) runtime: Handle,
    pub(crate) settings_window: Option<SettingsWindow>,
    pub(crate) last_proxy_stats_tick: Instant,
}

impl GuiApp {
    fn new(
        state: Arc<AppState>,
        tx: Sender<UiEvent>,
        proxy: EventLoopProxy<UiEvent>,
        runtime: Handle,
    ) -> Self {
        Self {
            state,
            tx,
            proxy,
            tray: None,
            menu_bar: None,
            server: None,
            runtime,
            settings_window: None,
            last_proxy_stats_tick: Instant::now(),
        }
    }

    pub(crate) fn refresh_ui(&mut self) {
        let snap = self.state.snapshot();
        if let Some(handles) = &mut self.tray {
            tray::refresh(handles, &snap);
        }
    }

    pub(crate) fn ensure_server(&mut self) -> Option<&Server> {
        if self.server.is_none() {
            match Server::start(self.state.clone(), self.tx.clone()) {
                Ok(s) => {
                    s.log().append(format!("settings ui served at {}", s.url()));
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

    pub(crate) fn append_log(&self, line: impl Into<String>) {
        crate::activity::activity_log().append(line);
    }
}

impl ApplicationHandler<UiEvent> for GuiApp {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) {
            return;
        }
        let drained: Vec<UiEvent> = self.tray.as_ref().map(tray::drain).unwrap_or_default();
        for ev in drained {
            dispatch::dispatch(self, event_loop, ev);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.tray.is_none() {
            let snap = self.state.snapshot();
            match tray::build(&snap) {
                Ok(mut handles) => {
                    if self.menu_bar.is_none() {
                        match menu::install(&mut handles.bindings) {
                            Ok(menu_handles) => self.menu_bar = Some(menu_handles),
                            Err(e) => diag(&format!("gui: menu bar init failed: {e}")),
                        }
                    }
                    self.tray = Some(handles);
                },
                Err(e) => diag(&format!("gui: tray init failed: {e}")),
            }
        }
        self.refresh_ui();
        dispatch::dispatch(self, event_loop, UiEvent::OpenSettings);
        let _ = self
            .proxy
            .send_event(UiEvent::GatewayProbeRequested { reply_to: None });

        hosts::tick::request_initial_probe(self);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UiEvent) {
        dispatch::dispatch(self, event_loop, event);
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let WindowEvent::ThemeChanged(theme) = &event {
            let label = match theme {
                winit::window::Theme::Light => "light",
                winit::window::Theme::Dark => "dark",
            };
            ipc_runtime::emit_theme_changed(self, label);
            return;
        }
        if let WindowEvent::CloseRequested = event
            && let Some(win) = &self.settings_window
            && win.id() == id
        {
            win.hide();
        }
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
            let _ = self
                .proxy
                .send_event(UiEvent::GatewayProbeRequested { reply_to: None });
        }

        hosts::tick::maybe_probe(self);

        if self.last_proxy_stats_tick.elapsed() >= Duration::from_secs(PROXY_STATS_TICK_SECS) {
            self.last_proxy_stats_tick = Instant::now();
            let _ = self.proxy.send_event(UiEvent::ProxyStatsTick);
        }

        event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_secs(1)));
    }
}
