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
    /// Doubles as the non-macOS tray icon source.
    pub window_icon_png: &'static [u8],
    pub tray_icon_png: &'static [u8],
    /// Appended last to the GUI `<head>` so its `:root` overrides win the
    /// cascade; empty for the default brand.
    pub theme_css: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Brand {
    pub app_name: &'static str,
    pub binary_name: &'static str,
    pub vendor: &'static str,
    pub config_dir: &'static str,
    pub config_file: &'static str,
    pub pat_file: &'static str,
    /// Working/state/cache/log leaf directory. Branding it isolates white-label
    /// builds from each other on disk.
    pub working_dir_name: &'static str,
    /// User-facing default Cowork workspace folder, created under the user's
    /// home (`~/<workspace_dir_name>`) and pushed as a pre-trusted
    /// `allowedWorkspaceFolders` entry so the agent gets a real writable
    /// working directory without folder prompts. Empty string ⇒ emit no
    /// default folder.
    pub workspace_dir_name: &'static str,
    pub keyring_service: &'static str,
    pub env_prefix: &'static str,
    pub default_gateway_url: &'static str,
    /// Gateway-relative consent-page path the session flow opens. Part of the
    /// deployment routing contract — must match where the gateway mounts it.
    pub device_link_path: &'static str,
    pub tray_tooltip: &'static str,
    pub window_title: &'static str,
    pub app_menu_name: &'static str,
    /// A full button label, not just the identity-provider name (e.g. "Sign in
    /// with Salesforce" for a Salesforce-federated gateway).
    pub sign_in_label: &'static str,
    pub sign_in_hint: &'static str,
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
        assets: BrandAssets {
            icon_svg: include_str!("../assets/icon.svg"),
            logo_svg: include_str!("../assets/logo.svg"),
            window_icon_png: include_bytes!("../assets/window-icon-1024.png"),
            tray_icon_png: include_bytes!("../assets/tray-icon.png"),
            theme_css: "",
        },
    };
}

static BRAND: OnceLock<&'static Brand> = OnceLock::new();

/// First writer wins; returns whether this call installed the brand.
pub fn set_brand(brand: &'static Brand) -> bool {
    BRAND.set(brand).is_ok()
}

#[must_use]
pub fn brand() -> &'static Brand {
    BRAND.get().copied().unwrap_or(&Brand::SYSTEMPROMPT)
}
