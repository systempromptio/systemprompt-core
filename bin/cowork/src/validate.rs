use crate::cache;
use crate::config;
use crate::http::GatewayClient;
use crate::paths::{self, Scope};
use std::process::ExitCode;

pub fn validate() -> ExitCode {
    let mut report = Report::new();

    report.info(
        "binary",
        &format!(
            "systemprompt-cowork v{} ({}-{})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    );

    match paths::org_plugins_effective() {
        Some(loc) => {
            let scope = match loc.scope {
                Scope::System => "system",
                Scope::User => "user",
            };
            report.ok(
                "org-plugins path",
                &format!("{} (scope: {scope})", loc.path.display()),
            );
            let meta = paths::metadata_dir(&loc.path);
            if meta.exists() {
                report.ok("metadata dir", &meta.display().to_string());
            } else {
                report.warn(
                    "metadata dir",
                    &format!("{} (missing — run `install`)", meta.display()),
                );
            }
            let last_sync = meta.join(paths::LAST_SYNC_SENTINEL);
            if last_sync.exists() {
                match std::fs::read_to_string(&last_sync) {
                    Ok(s) => report.ok("last sync", &summarise_last_sync(&s)),
                    Err(e) => report.warn("last sync", &format!("unreadable: {e}")),
                }
            } else {
                report.warn("last sync", "never — run `sync`");
            }

            match count_installed_plugins(&loc.path) {
                Some(n) => report.ok("plugins on disk", &format!("{n}")),
                None => report.warn("plugins on disk", "could not enumerate"),
            }
        },
        None => report.fail("org-plugins path", "unresolvable for this OS"),
    }

    let cfg = config::load();
    match cfg.gateway_url.as_deref() {
        Some(url) => {
            report.ok("gateway_url", url);
            let client = GatewayClient::new(url.to_string());
            match client.health() {
                Ok(()) => report.ok("gateway /health", "reachable"),
                Err(e) => report.fail("gateway /health", &e),
            }
        },
        None => report.fail("gateway_url", "not set in config"),
    }

    match cache::read_valid() {
        Some(out) => {
            report.ok(
                "cached token",
                &format!("ttl={}s, len={}", out.ttl, out.token.len()),
            );
        },
        None => {
            report.warn(
                "cached token",
                "absent or expired — helper will probe providers on next run",
            );
        },
    }

    match config::pinned_pubkey() {
        Some(k) => report.ok("pinned manifest pubkey", &format!("{} chars", k.len())),
        None => report.warn(
            "pinned manifest pubkey",
            "not pinned — run `install` or first sync will pin",
        ),
    }

    report.print();
    if report.any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn summarise_last_sync(raw: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return "unparseable".into();
    };
    let synced_at = value
        .get("synced_at")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let manifest_version = value
        .get("manifest_version")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let mcp_count = value
        .get("mcp_server_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    format!("{synced_at} (manifest {manifest_version}, {mcp_count} MCP server(s))")
}

fn count_installed_plugins(org_plugins: &std::path::Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(org_plugins).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().ok()?.is_dir() {
            n += 1;
        }
    }
    Some(n)
}

struct Report {
    any_failed: bool,
    lines: Vec<String>,
}

impl Report {
    fn new() -> Self {
        Self {
            any_failed: false,
            lines: Vec::new(),
        }
    }
    fn ok(&mut self, label: &str, value: &str) {
        self.lines.push(format!("  [ok]   {label}: {value}"));
    }
    fn warn(&mut self, label: &str, value: &str) {
        self.lines.push(format!("  [warn] {label}: {value}"));
    }
    fn fail(&mut self, label: &str, value: &str) {
        self.any_failed = true;
        self.lines.push(format!("  [fail] {label}: {value}"));
    }
    fn info(&mut self, label: &str, value: &str) {
        self.lines.push(format!("         {label}: {value}"));
    }
    fn print(&self) {
        println!("systemprompt-cowork validate");
        for line in &self.lines {
            println!("{line}");
        }
        if self.any_failed {
            println!("\nResult: FAIL — one or more critical checks did not pass.");
        } else {
            println!("\nResult: OK");
        }
    }
}
