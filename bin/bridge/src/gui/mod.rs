//! Desktop GUI: tray, menu, webview IPC, and event dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod assets;
pub mod command;
pub mod dispatch;
pub mod emit;
pub mod error;
pub mod events;
pub mod first_run;
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

use std::collections::VecDeque;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
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

// Why: winit 0.31 removed generic user events — an `EventLoopProxy` can only
// `wake_up()` the loop, carrying no payload, so the queue here is what actually
// transports a `UiEvent`.
#[derive(Clone, Debug)]
pub(crate) struct UiEventProxy {
    proxy: EventLoopProxy,
    queue: Arc<Mutex<VecDeque<UiEvent>>>,
}

impl UiEventProxy {
    pub(crate) fn new(proxy: EventLoopProxy) -> Self {
        Self {
            proxy,
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub(crate) fn send_event(&self, event: UiEvent) {
        self.queue.lock().push_back(event);
        self.proxy.wake_up();
    }

    fn drain(&self) -> Vec<UiEvent> {
        let mut q = self.queue.lock();
        std::mem::take(&mut *q).into_iter().collect()
    }
}

fn install_termination_handlers(proxy: UiEventProxy) {
    if let Err(e) = ctrlc::set_handler(move || {
        proxy.send_event(UiEvent::Quit);
    }) {
        tracing::warn!(error = %e, "ctrl-c handler not installed; quit signal unavailable");
    }
}

#[tracing::instrument]
pub fn run() -> ExitCode {
    let proxy_outcome = crate::proxy::start_default();

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            diag(&format!("gui: failed to build event loop: {e}"));
            return ExitCode::from(1);
        },
    };
    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = UiEventProxy::new(event_loop.create_proxy());
    install_termination_handlers(proxy.clone());
    emit::install_log_emitter(proxy.clone());
    let (tx, rx) = channel::<UiEvent>();

    let bridge_proxy = proxy.clone();
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            bridge_proxy.send_event(event);
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

    match &proxy_outcome {
        crate::proxy::StartOutcome::Started(h) => {
            app.append_log(format!("local proxy listening on 127.0.0.1:{}", h.port));
            if h.port != crate::proxy::DEFAULT_PROXY_PORT {
                app.append_log(format!(
                    "port {} was taken by another listener — host profiles written for it will be \
                     rejected until you re-apply them",
                    crate::proxy::DEFAULT_PROXY_PORT
                ));
            }
        },
        // Why: a sibling window of this same install already serves the port.
        // Keep running — the GUI is still useful against that proxy.
        crate::proxy::StartOutcome::AlreadyRunning {
            port, config_dir, ..
        } => {
            app.append_log(format!(
                "another {} bridge from {config_dir} is already serving 127.0.0.1:{port}; this \
                 window will use it",
                crate::brand::brand().app_name
            ));
        },
        crate::proxy::StartOutcome::Failed { tried, last_error } => {
            app.append_log(format!(
                "local proxy FAILED to start — host requests will be refused. Tried ports \
                 {tried:?}: {last_error}"
            ));
        },
    }

    if let Err(e) = event_loop.run_app(app) {
        diag(&format!("gui: event loop error: {e}"));
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

pub(crate) struct GuiApp {
    pub(crate) state: Arc<AppState>,
    pub(crate) tx: Sender<UiEvent>,
    pub(crate) proxy: UiEventProxy,
    pub(crate) tray: Option<tray::TrayHandles>,
    pub(crate) menu_bar: Option<menu::MenuBarHandles>,
    pub(crate) server: Option<Server>,
    pub(crate) runtime: Handle,
    pub(crate) settings_window: Option<SettingsWindow>,
    pub(crate) last_proxy_stats_tick: Instant,
    pub(crate) last_state_hash: Option<u64>,
    pub(crate) did_initial_sync: bool,
}

impl GuiApp {
    fn new(
        state: Arc<AppState>,
        tx: Sender<UiEvent>,
        proxy: UiEventProxy,
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
            last_state_hash: None,
            did_initial_sync: false,
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
            match Server::start(Arc::clone(&self.state), self.tx.clone()) {
                Ok(s) => {
                    Server::log().append(format!("settings ui served at {}", s.url()));
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

    #[expect(
        clippy::unused_self,
        reason = "method form keeps the app.append_log(..) call sites uniform across handlers"
    )]
    pub(crate) fn append_log(&self, line: impl Into<String>) {
        crate::activity::activity_log().append(line);
    }
}

impl ApplicationHandler for GuiApp {
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) {
            return;
        }
        let drained: Vec<UiEvent> = self.tray.as_ref().map(tray::drain).unwrap_or_default();
        for ev in drained {
            dispatch::dispatch(self, event_loop, ev);
        }
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
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
        self.proxy
            .send_event(UiEvent::GatewayProbeRequested { reply_to: None });

        hosts::tick::request_initial_probe(self);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        for event in self.proxy.drain() {
            dispatch::dispatch(self, event_loop, event);
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::ThemeChanged(theme) = &event {
            let label = match theme {
                winit::window::Theme::Light => "light",
                winit::window::Theme::Dark => "dark",
            };
            emit::emit_theme_changed(self, label);
            return;
        }
        if let WindowEvent::SurfaceResized(size) = &event
            && let Some(win) = &self.settings_window
            && win.id() == id
        {
            win.resize_webview(*size);
            return;
        }
        if event == WindowEvent::CloseRequested
            && let Some(win) = &self.settings_window
            && win.id() == id
        {
            win.hide();
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handles) = &self.tray {
            for ev in tray::drain(handles) {
                self.proxy.send_event(ev);
            }
        }
        let snap = self.state.snapshot();
        let needs_probe = matches!(snap.gateway_status, GatewayStatus::Unknown)
            || snap
                .last_probe_at_unix
                .is_none_or(|t| now_unix().saturating_sub(t) >= PROBE_INTERVAL_SECS);
        if needs_probe && !matches!(snap.gateway_status, GatewayStatus::Probing) {
            self.proxy
                .send_event(UiEvent::GatewayProbeRequested { reply_to: None });
        }

        if !self.did_initial_sync && snap.signed_in() && snap.pat_present && !snap.sync_in_flight {
            self.did_initial_sync = true;
            self.append_log("auto-sync on startup (rehydrating managed MCP registry)…");
            self.proxy
                .send_event(UiEvent::SyncRequested { reply_to: None });
        }

        hosts::tick::maybe_probe(self);
        first_run::tick(self);

        if self.last_proxy_stats_tick.elapsed() >= Duration::from_secs(PROXY_STATS_TICK_SECS) {
            self.last_proxy_stats_tick = Instant::now();
            self.proxy.send_event(UiEvent::ProxyStatsTick);
        }

        event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_secs(1)));
    }
}
