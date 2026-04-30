use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, Window, WindowAttributes, WindowId};
use wry::{NewWindowResponse, WebView, WebViewBuilder};

use crate::gui::error::{GuiError, GuiResult, WindowError};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

const WINDOW_ICON_PNG: &[u8] = include_bytes!("../../../assets/window-icon-1024.png");

const DEFAULT_WIDTH: u32 = 1100;
const DEFAULT_HEIGHT: u32 = 760;
const MIN_WIDTH: u32 = 800;
const MIN_HEIGHT: u32 = 600;
const BG_RGBA: (u8, u8, u8, u8) = (15, 17, 21, 255);

pub struct SettingsWindow {
    window: Window,
    _webview: WebView,
}

impl SettingsWindow {
    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn create(event_loop: &ActiveEventLoop, url: &str, port: u16) -> GuiResult<Self> {
        let attrs = chrome_attributes(
            Window::default_attributes()
                .with_title("systemprompt cowork")
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

        let local_origin = format!("http://127.0.0.1:{port}");
        let webview = WebViewBuilder::new()
            .with_url(url)
            .with_background_color(BG_RGBA)
            .with_accept_first_mouse(true)
            .with_navigation_handler(move |target| allow_navigation(&target, &local_origin))
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

        Ok(Self {
            window,
            _webview: webview,
        })
    }

    pub fn focus(&self) {
        self.window.set_visible(true);
        self.window.focus_window();
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }
}

fn allow_navigation(target: &str, local_origin: &str) -> bool {
    if target.starts_with(local_origin) || target.starts_with("about:") {
        return true;
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        super::open_external_url(target);
        return false;
    }
    true
}

fn decode_icon() -> Option<Icon> {
    let img = image::load_from_memory(WINDOW_ICON_PNG).ok()?.to_rgba8();
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
fn chrome_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs
}
