//! Native window construction: dimensions, decorations, platform quirks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::{Icon, Window, WindowAttributes, WindowId};
use wry::http::Response;
use wry::http::header::CONTENT_TYPE;
use wry::{NewWindowResponse, WebView, WebViewBuilder};

use crate::gui::assets::{self, Asset};
use crate::gui::error::{GuiError, GuiResult, WindowError};
use crate::gui::events::UiEvent;

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

const DEFAULT_WIDTH: u32 = 1100;
const DEFAULT_HEIGHT: u32 = 760;
const MIN_WIDTH: u32 = 800;
const MIN_HEIGHT: u32 = 600;
const BG_RGBA: (u8, u8, u8, u8) = (15, 17, 21, 255);

const SP_PROTOCOL: &str = "sp";
const SP_HOST: &str = "app";
#[cfg(any(target_os = "windows", target_os = "android"))]
const SP_INDEX_URL: &str = "http://sp.app/index.html";
#[cfg(not(any(target_os = "windows", target_os = "android")))]
const SP_INDEX_URL: &str = "sp://app/index.html";

pub struct SettingsWindow {
    window: Window,
    webview: WebView,
}

impl std::fmt::Debug for SettingsWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsWindow").finish_non_exhaustive()
    }
}

impl SettingsWindow {
    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub const fn winit_window(&self) -> &Window {
        &self.window
    }

    pub fn create(
        event_loop: &ActiveEventLoop,
        proxy: &EventLoopProxy<UiEvent>,
        legacy_origin: Option<&str>,
    ) -> GuiResult<Self> {
        let attrs = chrome_attributes(
            Window::default_attributes()
                .with_title(crate::brand::brand().window_title)
                .with_inner_size(LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT))
                .with_min_inner_size(PhysicalSize::new(MIN_WIDTH, MIN_HEIGHT))
                .with_visible(false)
                .with_window_icon(decode_icon()),
        );

        let window = event_loop
            .create_window(attrs)
            .map_err(|e| GuiError::Window {
                context: "create_window".into(),
                source: WindowError::Os(e),
            })?;

        let nav_legacy: Option<String> = legacy_origin.map(str::to_owned);
        let ipc_proxy = proxy.clone();
        let webview = WebViewBuilder::new()
            .with_url(SP_INDEX_URL)
            .with_background_color(BG_RGBA)
            .with_accept_first_mouse(true)
            .with_devtools(true)
            .with_initialization_script(BRIDGE_BOOTSTRAP)
            .with_ipc_handler(move |req| {
                let body = req.into_body();
                _ = ipc_proxy.send_event(UiEvent::IpcInbound(body));
            })
            .with_custom_protocol(SP_PROTOCOL.to_owned(), move |_id, request| {
                serve_custom_asset(&request)
            })
            .with_navigation_handler(move |target| allow_navigation(&target, nav_legacy.as_deref()))
            .with_new_window_req_handler(move |target, _features| {
                super::open_external_url(&target);
                NewWindowResponse::Deny
            })
            .build(&window)
            .map_err(|e| GuiError::Window {
                context: "webview build".into(),
                source: WindowError::Wry(e),
            })?;

        window.set_visible(true);
        window.focus_window();

        #[cfg(debug_assertions)]
        webview.open_devtools();

        Ok(Self { window, webview })
    }

    pub fn open_devtools(&self) {
        self.webview.open_devtools();
    }

    pub fn focus(&self) {
        self.window.set_visible(true);
        self.window.focus_window();
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn evaluate_script(&self, script: &str) {
        if let Err(e) = self.webview.evaluate_script(script) {
            tracing::warn!(error = %e, "evaluate_script failed");
        }
    }
}

const BRIDGE_BOOTSTRAP: &str = r#"
(function () {
  if (window.__bridge && window.__bridge.__installed) { return; }
  const pending = new Map();
  const subs = new Map();
  const bridge = {
    __installed: true,
    pending,
    subs,
    reply(id, payload) {
      const p = pending.get(id);
      if (!p) { return; }
      pending.delete(id);
      if (payload && payload.ok) { p.resolve(payload.value); }
      else { p.reject(payload && payload.error ? payload.error : { scope: "internal", code: "internal", message: "no payload" }); }
    },
    emit(channel, payload) {
      const set = subs.get(channel);
      if (!set) { return; }
      for (const cb of Array.from(set)) {
        try { cb(payload); } catch (e) { console.error("bridge subscriber threw", e); }
      }
    },
  };
  window.__bridge = bridge;
})();
"#;

fn serve_custom_asset(request: &http::Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let uri = request.uri();
    let host_match = uri.host().is_none_or(|h| h == SP_HOST);
    if !host_match {
        return not_found();
    }
    let mut path = uri.path().to_owned();
    if path.is_empty() || path == "/" {
        "/index.html".clone_into(&mut path);
    }
    assets::lookup_path(&path).map_or_else(not_found, asset_response)
}

fn asset_response(asset: Asset) -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(asset.body);
    _ = response.headers_mut().insert(
        CONTENT_TYPE,
        http::HeaderValue::from_str(asset.content_type)
            .unwrap_or_else(|_| http::HeaderValue::from_static("application/octet-stream")),
    );
    _ = response.headers_mut().insert(
        http::header::CACHE_CONTROL,
        http::HeaderValue::from_static("no-store, must-revalidate"),
    );
    response
}

fn not_found() -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(Cow::Borrowed::<'static, [u8]>(b"not found"));
    *response.status_mut() = http::StatusCode::NOT_FOUND;
    _ = response.headers_mut().insert(
        CONTENT_TYPE,
        http::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn allow_navigation(target: &str, legacy_origin: Option<&str>) -> bool {
    if target.starts_with("sp://")
        || target.starts_with("http://sp.app")
        || target.starts_with("about:")
    {
        return true;
    }
    if let Some(origin) = legacy_origin
        && target.starts_with(origin)
    {
        return true;
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        super::open_external_url(target);
        return false;
    }
    true
}

fn decode_icon() -> Option<Icon> {
    let img = image::load_from_memory(crate::brand::brand().assets.window_icon_png)
        .ok()?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

#[cfg(target_os = "macos")]
fn chrome_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs
        .with_titlebar_transparent(true)
        .with_title_hidden(true)
        .with_fullsize_content_view(true)
}

#[cfg(not(target_os = "macos"))]
const fn chrome_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs
}
