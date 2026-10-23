//! The settings webview: custom protocol, IPC bridge and navigation policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use winit::dpi::{LogicalPosition, PhysicalSize};
use winit::window::Window;
use wry::{NewWindowResponse, Rect, WebView, WebViewBuilder};

use super::native_protocol::{BRIDGE_BOOTSTRAP, serve_custom_asset};
use crate::gui::UiEventProxy;
use crate::gui::error::{GuiError, GuiResult, WindowError};
use crate::gui::events::UiEvent;
use crate::wire::external_url::ExternalUrl;

const BG_RGBA: (u8, u8, u8, u8) = (15, 17, 21, 255);
const SP_PROTOCOL: &str = "sp";
pub(super) const SP_HOST: &str = "app";
// Why: Chromium preloads .app HSTS, upgrading HTTP subresources past wry's
// interception filter.
#[cfg(any(target_os = "windows", target_os = "android"))]
const SP_INDEX_URL: &str = "https://sp.app/index.html";
#[cfg(not(any(target_os = "windows", target_os = "android")))]
const SP_INDEX_URL: &str = "sp://app/index.html";

// Why: winit returns Box<dyn Window>, but wry requires a sized HasWindowHandle.
struct WindowRef<'a>(&'a dyn Window);

impl raw_window_handle::HasWindowHandle for WindowRef<'_> {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.0.window_handle()
    }
}

pub(super) fn build_webview(window: &dyn Window, proxy: &UiEventProxy) -> GuiResult<WebView> {
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
        .with_navigation_handler(|target| allow_navigation(&target))
        .with_new_window_req_handler(|target, _features| {
            open_if_external(&target);
            NewWindowResponse::Deny
        })
        .build_as_child(&WindowRef(window))
        .map_err(|e| {
            let source = WindowError::Wry(e);
            // Why: windows_subsystem = "windows" provides no console for startup errors.
            crate::user_alert::alert_user(
                &format!("{} could not start", crate::brand::brand().app_name),
                &format!("The embedded browser failed to initialise: {source}"),
            );
            GuiError::Window {
                context: "webview build".into(),
                source,
            }
        })?;
    Ok(webview)
}

fn allow_navigation(target: &str) -> bool {
    if target.starts_with("sp://")
        || target.starts_with("http://sp.app")
        || target.starts_with("https://sp.app")
        || target.starts_with("about:")
    {
        return true;
    }
    open_if_external(target);
    false
}

fn open_if_external(target: &str) {
    match ExternalUrl::parse(target) {
        Ok(url) => super::open_external_url(&url),
        Err(e) => tracing::warn!(error = %e, "webview navigation refused"),
    }
}
