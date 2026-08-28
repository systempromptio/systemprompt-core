//! Desktop GUI: tray, menu, webview IPC, and event dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
pub mod notify;
pub mod onboarding;
pub mod server;
pub mod server_json;
pub mod server_marketplace;
pub mod server_util;
pub mod state;
pub mod tray;
pub mod webview2;
pub mod window;

mod app;

use std::collections::{HashSet, VecDeque};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::Instant;

use parking_lot::Mutex;
use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy};

use crate::gui::events::UiEvent;
use crate::gui::server::Server;
use crate::gui::state::AppState;
use crate::gui::window::SettingsWindow;
use crate::obs::output::diag;
use tokio::runtime::Handle;

pub(crate) const PROBE_INTERVAL_SECS: u64 = 30;

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
    emit::install_request_emitter(proxy.clone());
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
    let app = GuiApp::new(app_state, tx, proxy, runtime);

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
    #[cfg(target_os = "macos")]
    pub(crate) menu_bar: Option<menu::MenuBarHandles>,
    pub(crate) server: Option<Server>,
    pub(crate) runtime: Handle,
    pub(crate) settings_window: Option<SettingsWindow>,
    pub(crate) last_proxy_stats_tick: Instant,
    pub(crate) last_event_loop_pass: Instant,
    pub(crate) last_saved_geometry: Option<crate::window_state::WindowGeometry>,
    pub(crate) last_state_hash: Option<u64>,
    pub(crate) did_initial_sync: bool,
    pub(crate) active_signals: HashSet<notify::Signal>,
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
            #[cfg(target_os = "macos")]
            menu_bar: None,
            server: None,
            runtime,
            settings_window: None,
            last_proxy_stats_tick: Instant::now(),
            last_event_loop_pass: Instant::now(),
            last_saved_geometry: None,
            last_state_hash: None,
            did_initial_sync: false,
            active_signals: HashSet::new(),
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

    #[expect(
        clippy::unused_self,
        reason = "method form keeps the app.append_log(..) call sites uniform across handlers"
    )]
    pub(crate) fn append_log_warn(&self, line: impl Into<String>) {
        crate::activity::activity_log().append_warn(line);
    }

    #[expect(
        clippy::unused_self,
        reason = "method form keeps the app.append_log(..) call sites uniform across handlers"
    )]
    pub(crate) fn append_log_error(&self, line: impl Into<String>) {
        crate::activity::activity_log().append_error(line);
    }
}
