use crate::http::GatewayClient;
use crate::paths::{self, Scope};
use crate::{cache, config};

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

pub fn run() -> ValidationReport {
    let mut report = Report::new();
    check_binary(&mut report);
    check_org_plugins(&mut report);
    check_gateway(&mut report);
    check_cached_token(&mut report);
    check_pinned_pubkey(&mut report);
    report.into_report()
}

fn check_binary(report: &mut Report) {
    report.info(
        "binary",
        &format!(
            "systemprompt-cowork v{} ({}-{})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    );
}

fn check_org_plugins(report: &mut Report) {
    let Some(loc) = paths::org_plugins_effective() else {
        report.fail("org-plugins path", "unresolvable for this OS");
        return;
    };
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

    check_last_sync(report, &meta);

    match count_installed_plugins(&loc.path) {
        Some(n) => report.ok("plugins on disk", &format!("{n}")),
        None => report.warn("plugins on disk", "could not enumerate"),
    }
}

fn check_last_sync(report: &mut Report, meta: &std::path::Path) {
    let last_sync = meta.join(paths::LAST_SYNC_SENTINEL);
    if !last_sync.exists() {
        report.warn("last sync", "never — run `sync`");
        return;
    }
    match std::fs::read_to_string(&last_sync) {
        Ok(s) => report.ok("last sync", &summarise_last_sync(&s)),
        Err(e) => report.warn("last sync", &format!("unreadable: {e}")),
    }
}

fn check_gateway(report: &mut Report) {
    let cfg = config::load();
    let Some(url) = cfg.gateway_url.as_deref() else {
        report.fail("gateway_url", "not set in config");
        return;
    };
    report.ok("gateway_url", url);
    let client = GatewayClient::new(url.to_string());
    match client.health() {
        Ok(()) => report.ok("gateway /health", "reachable"),
        Err(e) => report.fail("gateway /health", &e.to_string()),
    }
}

fn check_cached_token(report: &mut Report) {
    match cache::read_valid() {
        Some(out) => report.ok(
            "cached token",
            &format!("ttl={}s, len={}", out.ttl, out.token.len()),
        ),
        None => report.warn(
            "cached token",
            "absent or expired — helper will probe providers on next run",
        ),
    }
}

fn check_pinned_pubkey(report: &mut Report) {
    match config::pinned_pubkey() {
        Some(k) => report.ok("pinned manifest pubkey", &format!("{} chars", k.len())),
        None => report.fail(
            "pinned manifest pubkey",
            "not pinned — provide it out of band via MDM (`install --apply --pubkey <base64>`) or \
             rerun `sync --allow-tofu`",
        ),
    }
}

fn summarise_last_sync(raw: &str) -> String {
    #[derive(serde::Deserialize)]
    struct LastSyncRecord {
        #[serde(default)]
        synced_at: Option<String>,
        #[serde(default)]
        manifest_version: Option<String>,
        #[serde(default)]
        mcp_server_count: Option<u64>,
    }

    let Ok(record) = serde_json::from_str::<LastSyncRecord>(raw) else {
        return "unparseable".into();
    };
    let synced_at = record.synced_at.as_deref().unwrap_or("unknown");
    let manifest_version = record.manifest_version.as_deref().unwrap_or("?");
    let mcp_count = record.mcp_server_count.unwrap_or(0);
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
