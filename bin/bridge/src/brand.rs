//! Compile-time brand seam.
//!
//! The bridge is shipped both as the default `systemprompt` binary and as
//! per-client white-label builds (e.g. an Astound-branded bridge). Everything
//! that is *brand-specific* — the app name, window/tray chrome, on-disk paths,
//! environment-variable prefix, default gateway, and the GUI assets — is
//! gathered here behind a single [`Brand`] value so a downstream binary crate
//! can supply its own without forking the source tree.
//!
//! A binary selects its brand once at startup via
//! [`crate::run_with_brand`], which stores it in a process-global `OnceLock`.
//! Call sites read it through [`brand()`], which falls back to
//! [`Brand::SYSTEMPROMPT`] when nothing has been set (keeps unit tests and any
//! early call path safe).
//!
//! Note on scope: identifiers that form part of the *wire contract* with the
//! gateway and the managed host apps — plugin ids, the Codex
//! marketplace/provider ids, the governance hook id — are deliberately NOT
//! brand fields. They must stay in lockstep with what the gateway emits in its
//! signed manifest, so changing them is a coordinated gateway+bridge change,
//! not a per-client cosmetic swap.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy)]
pub struct BrandAssets {
    pub icon_svg: &'static str,
    pub logo_svg: &'static str,
    pub window_icon_png: &'static [u8],
    pub tray_icon_png: &'static [u8],
    pub app_icon_ico: &'static [u8],
    pub theme_css: &'static str,
}

// Why: protocol compatibility is negotiated on the core bridge library's
// version line, never a white-label brand's own (a branded 0.1.x would read as
// ancient against the gateway's MIN_BRIDGE_VERSION and be rejected outright).
// `Brand::version` stays the display/update/asset version; this constant is
// what the manifest floor check and the heartbeat report.
pub const COMPAT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy)]
pub struct Brand {
    pub app_name: &'static str,
    pub binary_name: &'static str,
    pub version: &'static str,
    pub vendor: &'static str,
    pub config_dir: &'static str,
    pub config_file: &'static str,
    pub pat_file: &'static str,
    pub working_dir_name: &'static str,
    pub workspace_dir_name: &'static str,
    pub keyring_service: &'static str,
    pub env_prefix: &'static str,
    pub default_gateway_url: &'static str,
    pub device_link_path: &'static str,
    pub tray_tooltip: &'static str,
    pub window_title: &'static str,
    pub app_menu_name: &'static str,
    pub sign_in_label: &'static str,
    pub sign_in_hint: &'static str,
    // Why: linked verbatim by the setup footer and the main footer -- no path
    // is appended, so a brand must point this at a page that actually exists.
    pub docs_url: &'static str,
    pub contact_email: &'static str,
    // Why: the setup splash is a two-panel screen whose left column is the
    // brand's pitch. These carry it so a white-label build states its own value
    // proposition with no forked setup component -- head is the one-line claim,
    // body the supporting sentence beneath it.
    pub pitch_head: &'static str,
    pub pitch_body: &'static str,
    pub schedule_label: &'static str,
    pub schedule_unit: &'static str,
    pub schedule_task_name: &'static str,
    pub autostart_label: &'static str,
    pub autostart_task_name: &'static str,
    pub aumid: &'static str,
    // Why: a white-label brand whose palette is a single dark surface has no
    // light theme to offer, so following the OS colour scheme hands it a
    // half-light window — a light title bar over its own dark page, or a light
    // page its brand tokens were never written for. Such a brand pins the GUI
    // dark here; brands that do ship both themes leave this `false` and keep
    // following the OS.
    pub force_dark: bool,
    pub assets: BrandAssets,
}

impl Brand {
    #[must_use]
    pub fn env(&self, suffix: &str) -> String {
        format!("{}_{suffix}", self.env_prefix)
    }

    pub const SYSTEMPROMPT: Self = Self {
        app_name: "Systemprompt Bridge",
        binary_name: "systemprompt-bridge",
        version: env!("CARGO_PKG_VERSION"),
        vendor: "Systemprompt",
        config_dir: "systemprompt",
        config_file: "systemprompt-bridge.toml",
        pat_file: "systemprompt-bridge.pat",
        working_dir_name: "systemprompt-bridge",
        workspace_dir_name: "Systemprompt",
        keyring_service: "systemprompt-bridge.oauth-client",
        env_prefix: "SP_BRIDGE",
        default_gateway_url: "http://localhost:8080",
        device_link_path: "/bridge/device-link",
        tray_tooltip: "systemprompt-bridge",
        window_title: "systemprompt bridge",
        app_menu_name: "systemprompt-bridge",
        sign_in_label: "Sign in to your gateway",
        sign_in_hint: "Opens your browser to sign in on the gateway; this device is linked automatically.",
        docs_url: "https://systemprompt.io/documentation",
        contact_email: "ed@systemprompt.io",
        pitch_head: "Govern every coding agent.",
        pitch_body: "One gateway. Every agent. Every tool call audited.",
        schedule_label: "io.systemprompt.bridge-sync",
        schedule_unit: "systemprompt-bridge-sync",
        schedule_task_name: "SystempromptBridgeSync",
        autostart_label: "io.systemprompt.bridge-gui",
        autostart_task_name: "SystempromptBridgeGui",
        aumid: "io.systemprompt.bridge",
        force_dark: false,
        assets: BrandAssets {
            icon_svg: include_str!("../assets/icon.svg"),
            logo_svg: include_str!("../assets/logo.svg"),
            window_icon_png: include_bytes!("../assets/window-icon-1024.png"),
            tray_icon_png: include_bytes!("../assets/tray-icon.png"),
            app_icon_ico: include_bytes!("../assets/app-icon.ico"),
            theme_css: "",
        },
    };
}

static BRAND: OnceLock<&'static Brand> = OnceLock::new();

pub fn set_brand(brand: &'static Brand) -> bool {
    BRAND.set(brand).is_ok()
}

#[must_use]
pub fn brand() -> &'static Brand {
    BRAND.get().copied().unwrap_or(&Brand::SYSTEMPROMPT)
}
