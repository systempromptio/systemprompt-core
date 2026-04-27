use std::cell::RefCell;
use std::sync::mpsc::Sender;

use serde::Deserialize;
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};
use wry::{WebView, WebViewBuilder};

use crate::gui::events::UiEvent;
use crate::gui::state::{AppStateSnapshot, CachedToken};
use crate::output::diag;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HTML: &str = include_str!("../../web/index.html");
const STYLE: &str = include_str!("../../web/style.css");
const SCRIPT: &str = include_str!("../../web/app.js");
const ICON_SVG: &str = include_str!("../../assets/icon.svg");
const LOGO_SVG: &str = include_str!("../../assets/logo.svg");

pub struct WebWindow {
    tx: Sender<UiEvent>,
    window: RefCell<Option<Window>>,
    webview: RefCell<Option<WebView>>,
    pending_snapshot: RefCell<Option<AppStateSnapshot>>,
}

impl WebWindow {
    pub fn new(tx: Sender<UiEvent>) -> Self {
        Self {
            tx,
            window: RefCell::new(None),
            webview: RefCell::new(None),
            pending_snapshot: RefCell::new(None),
        }
    }

    pub fn show(&self, event_loop: &ActiveEventLoop, snapshot: &AppStateSnapshot) {
        if self.window.borrow().is_none() {
            if let Err(e) = self.build(event_loop) {
                diag(&format!("gui: webview build failed: {e}"));
                return;
            }
        }
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_visible(true);
            window.focus_window();
        }
        self.refresh(snapshot);
    }

    pub fn refresh(&self, snapshot: &AppStateSnapshot) {
        let json = snapshot_to_json(snapshot);
        if let Some(view) = self.webview.borrow().as_ref() {
            let script =
                format!("if (window.systemprompt) window.systemprompt.update({json});");
            let _ = view.evaluate_script(&script);
        } else {
            *self.pending_snapshot.borrow_mut() = Some(snapshot.clone());
        }
    }

    pub fn log(&self, message: &str) {
        if let Some(view) = self.webview.borrow().as_ref() {
            let escaped = serde_json::Value::String(message.to_string()).to_string();
            let script =
                format!("if (window.systemprompt) window.systemprompt.log({escaped});");
            let _ = view.evaluate_script(&script);
        }
    }

    pub fn close(&self) {
        if let Some(w) = self.window.borrow().as_ref() {
            w.set_visible(false);
        }
    }

    pub fn handle_window_event(&self, event: &winit::event::WindowEvent) {
        if let winit::event::WindowEvent::CloseRequested = event {
            if let Some(w) = self.window.borrow().as_ref() {
                w.set_visible(false);
            }
        }
    }

    pub fn owns(&self, id: winit::window::WindowId) -> bool {
        self.window
            .borrow()
            .as_ref()
            .map(|w| w.id() == id)
            .unwrap_or(false)
    }

    fn build(&self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let attrs = WindowAttributes::default()
            .with_title("systemprompt cowork")
            .with_inner_size(LogicalSize::new(880.0, 640.0))
            .with_min_inner_size(LogicalSize::new(640.0, 480.0))
            .with_visible(false);
        let window = event_loop
            .create_window(attrs)
            .map_err(|e| format!("create window: {e}"))?;

        let html = render_html();
        let tx = self.tx.clone();
        let webview = WebViewBuilder::new(&window)
            .with_html(&html)
            .with_ipc_handler(move |req| handle_ipc(&tx, &req.into_body()))
            .build()
            .map_err(|e| format!("webview build: {e}"))?;

        *self.window.borrow_mut() = Some(window);
        *self.webview.borrow_mut() = Some(webview);

        if let Some(snap) = self.pending_snapshot.borrow_mut().take() {
            self.refresh(&snap);
        }
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_visible(true);
            window.focus_window();
        }
        Ok(())
    }
}

fn render_html() -> String {
    HTML.replace("__STYLE__", STYLE)
        .replace("__SCRIPT__", SCRIPT)
        .replace("__VERSION__", VERSION)
        .replace("__ICON_SVG__", ICON_SVG)
        .replace("__LOGO_SVG__", LOGO_SVG)
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum IpcMessage {
    Ready,
    Sync,
    Validate,
    OpenFolder,
    Logout,
    Login {
        token: String,
        gateway: Option<String>,
    },
}

fn handle_ipc(tx: &Sender<UiEvent>, body: &str) {
    let msg: IpcMessage = match serde_json::from_str(body) {
        Ok(m) => m,
        Err(e) => {
            diag(&format!("gui: bad ipc payload {body:?}: {e}"));
            return;
        },
    };
    let event = match msg {
        IpcMessage::Ready => UiEvent::StateRefreshed,
        IpcMessage::Sync => UiEvent::SyncRequested,
        IpcMessage::Validate => UiEvent::ValidateRequested,
        IpcMessage::OpenFolder => UiEvent::OpenConfigFolder,
        IpcMessage::Logout => UiEvent::LogoutRequested,
        IpcMessage::Login { token, gateway } => UiEvent::LoginRequested { token, gateway },
    };
    let _ = tx.send(event);
}

fn snapshot_to_json(snap: &AppStateSnapshot) -> String {
    serde_json::json!({
        "identity": snap.identity,
        "gateway_url": snap.gateway_url,
        "config_file": snap.config_file,
        "pat_file": snap.pat_file,
        "config_present": snap.config_present,
        "pat_present": snap.pat_present,
        "plugins_dir": snap.plugins_dir,
        "last_sync_summary": snap.last_sync_summary,
        "skill_count": snap.skill_count,
        "agent_count": snap.agent_count,
        "sync_in_flight": snap.sync_in_flight,
        "last_action_message": snap.last_action_message,
        "cached_token": snap.cached_token.as_ref().map(cached_token_json),
    })
    .to_string()
}

fn cached_token_json(t: &CachedToken) -> serde_json::Value {
    serde_json::json!({
        "ttl_seconds": t.ttl_seconds,
        "length": t.length,
    })
}
