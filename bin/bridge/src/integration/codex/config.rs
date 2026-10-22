//! Codex CLI `config.toml` fragment generation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const BINARY: &str = "codex";

pub(super) const PROVIDER_NAME: &str = "model_providers.systemprompt.name";
pub(super) const PROVIDER_BASE_URL: &str = "model_providers.systemprompt.base_url";
pub(super) const PROVIDER_WIRE_API: &str = "model_providers.systemprompt.wire_api";
pub(super) const PROVIDER_AUTH_COMMAND: &str = "model_providers.systemprompt.auth.command";
pub(super) const PROVIDER_AUTH_ARGS: &str = "model_providers.systemprompt.auth.args";
pub(super) const PROVIDER_AUTH_TIMEOUT: &str = "model_providers.systemprompt.auth.timeout_ms";
pub(super) const PROVIDER_MODELS: &str = "model_providers.systemprompt.models";
pub(super) const PROVIDER_AUTH_REFRESH: &str =
    "model_providers.systemprompt.auth.refresh_interval_ms";
pub(super) const PROVIDER_HEADER_TENANT: &str =
    "model_providers.systemprompt.http_headers.x-tenant";
pub(super) const OTEL_LOG_USER_PROMPT: &str = "otel.log_user_prompt";
pub(super) const OTEL_ENDPOINT: &str = "otel.exporter.otlp-http.endpoint";
pub(super) const OTEL_PROTOCOL: &str = "otel.exporter.otlp-http.protocol";
pub(super) const ANALYTICS_ENABLED: &str = "analytics.enabled";
pub(super) const TOP_MODEL_PROVIDER: &str = "model_provider";
pub(super) const APPROVAL_POLICY: &str = "approval_policy";
pub(super) const SANDBOX_MODE: &str = "sandbox_mode";
pub(super) const SANDBOX_NETWORK_ACCESS: &str = "sandbox_workspace_write.network_access";

pub(super) const APPROVAL_POLICY_VALUE: &str = "never";
pub(super) const SANDBOX_MODE_VALUE: &str = "workspace-write";

pub(super) const KEYS_OF_INTEREST: &[&str] = &[
    TOP_MODEL_PROVIDER,
    PROVIDER_NAME,
    PROVIDER_BASE_URL,
    PROVIDER_WIRE_API,
    PROVIDER_AUTH_COMMAND,
    PROVIDER_AUTH_ARGS,
    PROVIDER_AUTH_TIMEOUT,
    PROVIDER_AUTH_REFRESH,
    PROVIDER_HEADER_TENANT,
    PROVIDER_MODELS,
    OTEL_LOG_USER_PROMPT,
    OTEL_ENDPOINT,
    OTEL_PROTOCOL,
    ANALYTICS_ENABLED,
    APPROVAL_POLICY,
    SANDBOX_MODE,
    SANDBOX_NETWORK_ACCESS,
];

#[must_use]
pub(super) fn role_of(key: &str) -> crate::drift::EntryRole {
    use crate::drift::EntryRole;
    match key {
        PROVIDER_BASE_URL => EntryRole::Endpoint,
        PROVIDER_MODELS => EntryRole::Models,
        _ => EntryRole::Setting,
    }
}

pub(super) fn codex_home() -> PathBuf {
    if let Some(custom) = std::env::var_os("CODEX_HOME") {
        return PathBuf::from(custom);
    }
    if let Some(home) = crate::basedirs::home_dir() {
        return home.join(".codex");
    }
    PathBuf::from(".codex")
}

pub(super) fn user_config_path() -> PathBuf {
    codex_home().join("config.toml")
}

// Why: Codex's Windows managed config is user-scoped under CODEX_HOME.
pub(super) fn managed_config_path() -> PathBuf {
    if let Some(custom) = std::env::var_os("CODEX_SYSTEM_CONFIG") {
        return PathBuf::from(custom);
    }
    if cfg!(target_os = "windows") {
        codex_home().join("managed_config.toml")
    } else {
        PathBuf::from("/etc/codex/config.toml")
    }
}

// Why: Codex's macOS MDM payload stores base64 TOML under config_toml_base64 in
// com.openai.codex preferences.
#[cfg(target_os = "macos")]
pub(super) fn macos_managed_prefs_paths() -> Vec<PathBuf> {
    const DOMAIN: &str = "com.openai.codex.plist";
    const ROOT: &str = "/Library/Managed Preferences";
    let mut paths = Vec::new();
    if let Some(user) = std::env::var_os("USER") {
        paths.push(PathBuf::from(ROOT).join(user).join(DOMAIN));
    }
    paths.push(PathBuf::from(ROOT).join(DOMAIN));
    paths
}

// Why: a GUI process launched from the desktop does not inherit the shell's
// PATH, so the install prefixes npm and Homebrew put the CLI under are probed
// as well.
pub(super) fn extra_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Some(home) = crate::basedirs::home_dir() {
        dirs.push(home.join(".npm-global").join("bin"));
        dirs.push(home.join(".bun").join("bin"));
        dirs.push(home.join("scoop").join("shims"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs
}

// Why: the Codex desktop app is a per-user install under
// `%LOCALAPPDATA%\Programs\Codex`; a process running from there, or from a
// macOS bundle, is the app packaging rather than the CLI of the same name.
pub(super) const APP_PROCESS_MARKERS: &[&str] = &["\\programs\\codex\\"];

pub(super) fn desktop_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        out.push(
            PathBuf::from(local)
                .join("Programs")
                .join("Codex")
                .join("Codex.exe"),
        );
    }
    out
}

pub(super) const fn desktop_locator(
    candidates: &[PathBuf],
) -> crate::integration::app_launch::AppLocator<'_> {
    crate::integration::app_launch::AppLocator {
        macos_name: "Codex",
        windows_name: "Codex",
        windows_candidates: candidates,
        linux_bin: BINARY,
        msix_family: None,
        msix_app_id: "App",
    }
}

pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub(super) fn redact_if_sensitive(key: &str, raw: String) -> String {
    if key == PROVIDER_HEADER_TENANT {
        let len = raw.chars().filter(|c| !c.is_whitespace()).count();
        return format!("<present, {len} chars>");
    }
    raw
}
