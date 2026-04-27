use crate::cache;
use crate::config;
use crate::http::GatewayClient;
use crate::paths::{self, Scope};
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckLevel {
    Ok,
    Warn,
    Fail,
    Info,
}

#[derive(Debug, Clone)]
pub struct CheckLine {
    pub level: CheckLevel,
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub lines: Vec<CheckLine>,
    pub any_failed: bool,
}

impl ValidationReport {
    pub fn rendered(&self) -> String {
        let mut s = String::from("systemprompt-cowork validate\n");
        for line in &self.lines {
            let prefix = match line.level {
                CheckLevel::Ok => "  [ok]   ",
                CheckLevel::Warn => "  [warn] ",
                CheckLevel::Fail => "  [fail] ",
                CheckLevel::Info => "         ",
            };
            s.push_str(prefix);
            s.push_str(&line.label);
            s.push_str(": ");
            s.push_str(&line.value);
            s.push('\n');
        }
        if self.any_failed {
            s.push_str("\nResult: FAIL — one or more critical checks did not pass.\n");
        } else {
            s.push_str("\nResult: OK\n");
        }
        s
    }
}

pub fn validate() -> ExitCode {
    let report = run();
    print!("{}", report.rendered());
    if report.any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

pub fn run() -> ValidationReport {
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
                Err(e) => report.fail("gateway /health", &e.to_string()),
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

    report.into_report()
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
    lines: Vec<CheckLine>,
}

impl Report {
    fn new() -> Self {
        Self {
            any_failed: false,
            lines: Vec::new(),
        }
    }
    fn ok(&mut self, label: &str, value: &str) {
        self.lines.push(CheckLine {
            level: CheckLevel::Ok,
            label: label.into(),
            value: value.into(),
        });
    }
    fn warn(&mut self, label: &str, value: &str) {
        self.lines.push(CheckLine {
            level: CheckLevel::Warn,
            label: label.into(),
            value: value.into(),
        });
    }
    fn fail(&mut self, label: &str, value: &str) {
        self.any_failed = true;
        self.lines.push(CheckLine {
            level: CheckLevel::Fail,
            label: label.into(),
            value: value.into(),
        });
    }
    fn info(&mut self, label: &str, value: &str) {
        self.lines.push(CheckLine {
            level: CheckLevel::Info,
            label: label.into(),
            value: value.into(),
        });
    }
    fn into_report(self) -> ValidationReport {
        ValidationReport {
            lines: self.lines,
            any_failed: self.any_failed,
        }
    }
}
