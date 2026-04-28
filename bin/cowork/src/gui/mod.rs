pub mod dispatch;
pub mod events;
pub mod handlers;
pub mod server;
pub mod server_json;
pub mod server_util;
pub mod state;
pub mod tray;
pub mod window;
pub mod worker;

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
use crate::gui::state::{AppState, GatewayStatus, now_unix};
use crate::gui::worker::WorkerPool;
use crate::output::diag;

const PROBE_INTERVAL_SECS: u64 = 30;

#[cfg(unix)]
fn install_termination_handlers() {
    extern "C" fn handle(signum: libc::c_int) {
        let code = 128 + signum;
        unsafe { libc::_exit(code) };
    }
    unsafe {
        libc::signal(libc::SIGINT, handle as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, handle as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, handle as *const () as libc::sighandler_t);
    }
}

#[cfg(not(unix))]
fn install_termination_handlers() {}

pub fn run() -> ExitCode {
    install_termination_handlers();

    crate::proxy::start_default();

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

pub(crate) struct GuiApp {
    pub(crate) state: Arc<AppState>,
    pub(crate) tx: Sender<UiEvent>,
    pub(crate) proxy: EventLoopProxy<UiEvent>,
    pub(crate) tray: Option<tray::TrayHandles>,
    pub(crate) server: Option<Server>,
    pub(crate) pool: WorkerPool,
}

impl GuiApp {
    fn new(state: Arc<AppState>, tx: Sender<UiEvent>, proxy: EventLoopProxy<UiEvent>) -> Self {
        Self {
            state,
            tx,
            proxy,
            tray: None,
            server: None,
            pool: WorkerPool::new(),
        }
    }

    pub(crate) fn refresh_ui(&mut self) {
        let snap = self.state.snapshot();
        if let Some(handles) = &self.tray {
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

    pub(crate) fn log(&self) -> Option<&ActivityLog> {
        self.server.as_ref().map(|s| s.log())
    }

    pub(crate) fn append_log(&self, line: impl Into<String>) {
        if let Some(log) = self.log() {
            log.append(line);
        }
    }
}

impl ApplicationHandler<UiEvent> for GuiApp {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) {
            return;
        }
        let drained: Vec<UiEvent> = self.tray.as_ref().map(tray::drain).unwrap_or_default();
        for ev in drained {
            dispatch::dispatch(self, ev);
        }
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.tray.is_none() {
            let snap = self.state.snapshot();
            match tray::build(&snap) {
                Ok(handles) => self.tray = Some(handles),
                Err(e) => diag(&format!("gui: tray init failed: {e}")),
            }
        }
        self.refresh_ui();
        dispatch::dispatch(self, UiEvent::OpenSettings);
        let _ = self.proxy.send_event(UiEvent::GatewayProbeRequested);
        #[cfg(target_os = "macos")]
        {
            let _ = self.proxy.send_event(UiEvent::ClaudeProbeRequested);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UiEvent) {
        dispatch::dispatch(self, event);
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}

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
            if claude_due && !snap.claude_probe_in_flight {
                let _ = self.proxy.send_event(UiEvent::ClaudeProbeRequested);
            }
        }
        event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_secs(1)));
    }
}
