use std::cell::RefCell;
use std::sync::mpsc::Sender;

use serde::Deserialize;
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};
use wry::http::{Request as HttpRequest, Response as HttpResponse, header};
use wry::{WebView, WebViewBuilder};

use crate::gui::events::UiEvent;
use crate::gui::state::{AppStateSnapshot, CachedToken};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HTML: &str = include_str!("../../web/index.html");
const STYLE: &str = include_str!("../../web/style.css");
const SCRIPT: &str = include_str!("../../web/app.js");
const ICON_SVG: &[u8] = include_bytes!("../../assets/icon.svg");
const LOGO_SVG: &[u8] = include_bytes!("../../assets/logo.svg");
const LOGO_DARK_SVG: &[u8] = include_bytes!("../../assets/logo-dark.svg");

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
                eprintln!("gui: webview build failed: {e}");
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

        let tx = self.tx.clone();
        let webview = WebViewBuilder::new(&window)
            .with_url("app://localhost/")
            .with_custom_protocol("app".into(), serve_asset)
            .with_ipc_handler(move |req| {
                let body: String = req.into_body();
                handle_ipc(&tx, &body);
            })
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

fn serve_asset(_req: HttpRequest<Vec<u8>>) -> HttpResponse<std::borrow::Cow<'static, [u8]>> {
    let uri = _req.uri();
    let path = uri.path();
    let (mime, body): (&'static str, std::borrow::Cow<'static, [u8]>) = match path {
        "/" | "" | "/index.html" => (
            "text/html; charset=utf-8",
            std::borrow::Cow::Owned(HTML.replace("__VERSION__", VERSION).into_bytes()),
        ),
        "/assets/style.css" => ("text/css; charset=utf-8", std::borrow::Cow::Borrowed(STYLE.as_bytes())),
        "/assets/app.js" => (
            "application/javascript; charset=utf-8",
            std::borrow::Cow::Borrowed(SCRIPT.as_bytes()),
        ),
        "/assets/icon.svg" => ("image/svg+xml", std::borrow::Cow::Borrowed(ICON_SVG)),
        "/assets/logo.svg" => ("image/svg+xml", std::borrow::Cow::Borrowed(LOGO_SVG)),
        "/assets/logo-dark.svg" => ("image/svg+xml", std::borrow::Cow::Borrowed(LOGO_DARK_SVG)),
        _ => (
            "text/plain; charset=utf-8",
            std::borrow::Cow::Borrowed(b"not found" as &[u8]),
        ),
    };
    let status = if matches!(path, "/" | "" | "/index.html" | "/assets/style.css"
        | "/assets/app.js" | "/assets/icon.svg" | "/assets/logo.svg" | "/assets/logo-dark.svg")
    {
        200
    } else {
        404
    };
    HttpResponse::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header("Cache-Control", "no-store")
        .body(body)
        .unwrap()
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
            eprintln!("gui: bad ipc payload {body:?}: {e}");
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
