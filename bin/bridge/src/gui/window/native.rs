//! Native window construction: dimensions, decorations, platform quirks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use winit::dpi::{LogicalPosition, LogicalSize, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::icon::{Icon, RgbaIcon};
use winit::window::{Window, WindowAttributes, WindowId};
use wry::http::Response;
use wry::http::header::CONTENT_TYPE;
use wry::{NewWindowResponse, Rect, WebView, WebViewBuilder};

use crate::gui::UiEventProxy;
use crate::gui::assets::{self, Asset};
use crate::gui::error::{GuiError, GuiResult, WindowError};
use crate::gui::events::UiEvent;

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesMacOS;

const DEFAULT_WIDTH: u32 = 1100;
const DEFAULT_HEIGHT: u32 = 760;
const MIN_WIDTH: u32 = 800;
const MIN_HEIGHT: u32 = 600;
const BG_RGBA: (u8, u8, u8, u8) = (15, 17, 21, 255);

const SP_PROTOCOL: &str = "sp";
const SP_HOST: &str = "app";
// Why: `.app` is HSTS-preloaded in Chromium, so WebView2 upgrades an http
// origin's subresources to https, past wry's interception filter.
#[cfg(any(target_os = "windows", target_os = "android"))]
const SP_INDEX_URL: &str = "https://sp.app/index.html";
#[cfg(not(any(target_os = "windows", target_os = "android")))]
const SP_INDEX_URL: &str = "sp://app/index.html";

pub struct SettingsWindow {
    window: Box<dyn Window>,
    webview: WebView,
}

// Why: winit 0.31's `create_window` returns an unsized `Box<dyn Window>`, but
// wry's `WebViewBuilder::build` needs a sized `HasWindowHandle`.
struct WindowRef<'a>(&'a dyn Window);

impl raw_window_handle::HasWindowHandle for WindowRef<'_> {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.0.window_handle()
    }
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

    pub fn winit_window(&self) -> &dyn Window {
        &*self.window
    }

    pub fn create(
        event_loop: &dyn ActiveEventLoop,
        proxy: &UiEventProxy,
        legacy_origin: Option<&str>,
    ) -> GuiResult<Self> {
        let attrs = chrome_attributes(
            WindowAttributes::default()
                .with_title(crate::brand::brand().window_title)
                .with_surface_size(LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT))
                .with_min_surface_size(PhysicalSize::new(MIN_WIDTH, MIN_HEIGHT))
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
        let builder = WebViewBuilder::new();
        #[cfg(target_os = "windows")]
        let builder = {
            use wry::WebViewBuilderExtWindows as _;
            builder.with_https_scheme(true)
        };
        let initial_size = window.surface_size();
        let webview = builder
            .with_url(SP_INDEX_URL)
            .with_background_color(BG_RGBA)
            .with_accept_first_mouse(true)
            .with_devtools(true)
            .with_bounds(Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: PhysicalSize::new(initial_size.width, initial_size.height).into(),
            })
            .with_initialization_script(BRIDGE_BOOTSTRAP)
            .with_ipc_handler(move |req| {
                let body = req.into_body();
                ipc_proxy.send_event(UiEvent::IpcInbound(body));
            })
            .with_custom_protocol(SP_PROTOCOL.to_owned(), move |_id, request| {
                serve_custom_asset(&request)
            })
            .with_navigation_handler(move |target| allow_navigation(&target, nav_legacy.as_deref()))
            .with_new_window_req_handler(move |target, _features| {
                super::open_external_url(&target);
                NewWindowResponse::Deny
            })
            .build_as_child(&WindowRef(&*window))
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

    /// Must be driven from `WindowEvent::Resized`: the webview is a child view
    /// with fixed bounds, so nothing resizes it otherwise.
    pub fn resize_webview(&self, size: PhysicalSize<u32>) {
        if let Err(e) = self.webview.set_bounds(Rect {
            position: LogicalPosition::new(0, 0).into(),
            size: PhysicalSize::new(size.width, size.height).into(),
        }) {
            tracing::warn!(error = %e, "webview set_bounds failed");
        }
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
    assets::lookup_path(&path).map_or_else(
        || {
            tracing::warn!(%path, "GUI asset not found; serving 404");
            not_found()
        },
        asset_response,
    )
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
    _ = response.headers_mut().insert(
        http::header::X_CONTENT_TYPE_OPTIONS,
        http::HeaderValue::from_static("nosniff"),
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
        || target.starts_with("https://sp.app")
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
    let img = match image::load_from_memory(crate::brand::brand().assets.window_icon_png) {
        Ok(img) => img.to_rgba8(),
        Err(e) => {
            tracing::warn!(error = %e, "window icon PNG failed to decode; running without one");
            return None;
        },
    };
    let (w, h) = img.dimensions();
    match RgbaIcon::new(img.into_raw(), w, h) {
        Ok(icon) => Some(icon.into()),
        Err(e) => {
            tracing::warn!(error = %e, "window icon rejected by winit; running without one");
            None
        },
    }
}

#[cfg(target_os = "macos")]
fn chrome_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs.with_platform_attributes(Box::new(
        WindowAttributesMacOS::default()
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true),
    ))
}

#[cfg(not(target_os = "macos"))]
fn chrome_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs
}
