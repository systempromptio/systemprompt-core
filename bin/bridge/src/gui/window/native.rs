//! Native window construction: dimensions, decorations, platform quirks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use winit::dpi::{LogicalPosition, LogicalSize, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::icon::{Icon, RgbaIcon};
use winit::window::{Window, WindowAttributes, WindowId};
use wry::{NewWindowResponse, Rect, WebView, WebViewBuilder};

use super::native_protocol::{BRIDGE_BOOTSTRAP, serve_custom_asset};
use crate::gui::UiEventProxy;
use crate::gui::error::{GuiError, GuiResult, WindowError};
use crate::gui::events::UiEvent;
use crate::window_state::{self as geometry, MIN_HEIGHT, MIN_WIDTH, WindowGeometry};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesMacOS;

const DEFAULT_WIDTH: u32 = 1100;
const DEFAULT_HEIGHT: u32 = 760;
const BG_RGBA: (u8, u8, u8, u8) = (15, 17, 21, 255);

const SP_PROTOCOL: &str = "sp";
pub(super) const SP_HOST: &str = "app";
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

    pub(crate) fn create(
        event_loop: &dyn ActiveEventLoop,
        proxy: &UiEventProxy,
        legacy_origin: Option<&str>,
    ) -> GuiResult<Self> {
        let mut attrs = chrome_attributes(
            WindowAttributes::default()
                .with_title(crate::brand::brand().window_title)
                .with_surface_size(LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT))
                .with_min_surface_size(LogicalSize::new(MIN_WIDTH, MIN_HEIGHT))
                .with_visible(false)
                .with_window_icon(decode_icon()),
        );

        let restored = geometry::load().and_then(|saved| {
            let areas = work_areas(event_loop);
            geometry::restore(saved, &areas)
        });
        if let Some(geom) = restored {
            attrs = attrs
                .with_position(LogicalPosition::new(geom.x, geom.y))
                .with_surface_size(LogicalSize::new(geom.width, geom.height));
        }

        let window = event_loop
            .create_window(attrs)
            .map_err(|e| GuiError::Window {
                context: "create_window".into(),
                source: WindowError::Os(e),
            })?;

        // Why: the web UI follows `prefers-color-scheme` and has a real light
        // theme, so pinning the title bar dark would reproduce the mismatch
        // this call exists to fix, with the colours swapped.
        super::set_immersive_dark(&*window, super::prefers_dark(&*window));
        if restored.is_some_and(|g| g.maximized) {
            window.set_maximized(true);
        }

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
            .with_devtools(cfg!(debug_assertions))
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
            .map_err(|e| {
                // Why: `windows_subsystem = "windows"` means a failure here has
                // no console to print to. Without this the app simply does not
                // appear — the commonest cause being a missing WebView2 runtime.
                super::alert_user(
                    &format!("{} could not start", crate::brand::brand().app_name),
                    &format!("The embedded browser failed to initialise: {e}"),
                );
                GuiError::Window {
                    context: "webview build".into(),
                    source: WindowError::Wry(e),
                }
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

    pub fn resize_webview(&self, size: PhysicalSize<u32>) {
        if let Err(e) = self.webview.set_bounds(Rect {
            position: LogicalPosition::new(0, 0).into(),
            size: PhysicalSize::new(size.width, size.height).into(),
        }) {
            tracing::warn!(error = %e, "webview set_bounds failed");
        }
    }

    #[must_use]
    pub fn current_geometry(&self) -> Option<WindowGeometry> {
        let scale = self.window.scale_factor();
        let pos = self.window.outer_position().ok()?.to_logical::<i32>(scale);
        let size = self.window.surface_size().to_logical::<u32>(scale);
        let (width, height) = geometry::clamp_size(size.width, size.height);
        Some(WindowGeometry {
            x: pos.x,
            y: pos.y,
            width,
            height,
            maximized: self.window.is_maximized(),
        })
    }

    pub fn evaluate_script(&self, script: &str) {
        if let Err(e) = self.webview.evaluate_script(script) {
            tracing::warn!(error = %e, "evaluate_script failed");
        }
    }
}


fn work_areas(event_loop: &dyn ActiveEventLoop) -> Vec<geometry::WorkArea> {
    event_loop
        .available_monitors()
        .map(|monitor| {
            let scale = monitor.scale_factor();
            let pos = monitor
                .position()
                .map_or(LogicalPosition::new(0, 0), |p| p.to_logical::<i32>(scale));
            let size = monitor
                .current_video_mode()
                .map_or(LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT), |m| {
                    m.size().to_logical::<u32>(scale)
                });
            geometry::WorkArea {
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
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
const fn chrome_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs
}
