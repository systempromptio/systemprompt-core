//! The local state a diagnostics bundle must carry to explain a broken proxy:
//! which process owns the port, what the key files look like, and what the
//! machine policy currently says.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use crate::context::BridgeContext;
use crate::proxy::peer::{self, PeerIdentity};
use crate::proxy::{DEFAULT_PROXY_PORT, ProxyRole};

#[must_use]
pub fn render(ctx: &BridgeContext) -> String {
    let brand = crate::brand::brand();
    let mut out: Vec<String> = Vec::new();
    out.push(format!(
        "{} {} ({})",
        brand.binary_name,
        brand.version,
        crate::buildinfo::short_sha()
    ));
    out.push(String::new());
    out.push("proxy:".to_owned());
    out.push(format!("  install id: {}", ctx.install_id().as_str()));
    out.push(format!("  role:       {}", describe_role(ctx.proxy.role())));
    out.push(format!("  endpoint:   {}", ctx.proxy.loopback().origin()));
    out.push(format!(
        "  secret fp:  {}",
        ctx.proxy
            .loopback()
            .secret_fingerprint()
            .unwrap_or_else(|| "<unavailable>".to_owned())
    ));
    out.push(format!(
        "  port {DEFAULT_PROXY_PORT}: {}",
        describe_peer(peer::probe_identity(DEFAULT_PROXY_PORT, ctx.install_id()))
    ));
    for fault in &ctx.startup_faults {
        out.push(format!("  startup fault: {fault}"));
    }
    out.push(String::new());
    out.push("port file:".to_owned());
    match crate::proxy::portfile::portfile_path() {
        Some(path) => append_file(&mut out, &path),
        None => {
            out.push("  <no config dir>".to_owned());
        },
    }
    out.push(String::new());
    out.push("config dir:".to_owned());
    match crate::config::config_path().and_then(|p| p.parent().map(Path::to_path_buf)) {
        Some(dir) => append_dir(&mut out, &dir),
        None => {
            out.push("  <no config dir>".to_owned());
        },
    }
    out.push(String::new());
    out.push("claude desktop policy:".to_owned());
    for line in crate::integration::claude_desktop::policy_summary() {
        out.push(format!("  {line}"));
    }
    out.push(String::new());
    out.push("bridge processes:".to_owned());
    let mut any = false;
    for proc_info in crate::sysproc::list_processes() {
        if !proc_info
            .name
            .to_ascii_lowercase()
            .contains(&brand.binary_name.to_ascii_lowercase())
        {
            continue;
        }
        any = true;
        out.push(format!(
            "  {} {}",
            proc_info.name,
            proc_info.path.as_deref().unwrap_or("<path unavailable>")
        ));
    }
    if !any {
        out.push("  <none visible to this account>".to_owned());
    }
    out.join("\n") + "\n"
}

fn describe_role(role: &ProxyRole) -> String {
    match role {
        ProxyRole::Serving(served) => format!("serving 127.0.0.1:{}", served.port),
        ProxyRole::Attached => "attached (this process does not serve)".to_owned(),
        ProxyRole::AlreadyRunning {
            port,
            pid,
            config_dir,
        } => format!("sibling pid {pid} serves 127.0.0.1:{port} from {config_dir}"),
        ProxyRole::Failed { tried, last_error } => {
            format!("FAILED (tried ports {tried:?}): {last_error}")
        },
    }
}

fn describe_peer(peer: PeerIdentity) -> String {
    match peer {
        PeerIdentity::Ours(who) => format!("ours (pid {}, {})", who.pid, who.config_dir),
        PeerIdentity::Foreign(who) => format!(
            "ANOTHER INSTALL (pid {}, {}) — another account on this computer runs the bridge",
            who.pid, who.config_dir
        ),
        PeerIdentity::Unknown => "held by an unidentified listener".to_owned(),
        PeerIdentity::Unreachable => "nothing listening".to_owned(),
    }
}

fn append_file(out: &mut Vec<String>, path: &Path) {
    out.push(format!("  {}", path.display()));
    match std::fs::read_to_string(path) {
        Ok(body) => {
            for line in body.lines() {
                out.push(format!("    {line}"));
            }
        },
        Err(e) => {
            out.push(format!("    <{e}>"));
        },
    }
}

fn append_dir(out: &mut Vec<String>, dir: &Path) {
    out.push(format!("  {}", dir.display()));
    out.push(format!("    {}", describe_access(dir)));
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            out.push(format!("    <{e}>"));
            return;
        },
    };
    let mut names: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    names.sort();
    for path in names {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let size = std::fs::metadata(&path).map_or_else(
            |e| format!("<{e}>"),
            |m| {
                if m.is_dir() {
                    "dir".to_owned()
                } else {
                    format!("{} bytes", m.len())
                }
            },
        );
        let readable = if path.is_dir() {
            String::new()
        } else {
            match std::fs::File::open(&path) {
                Ok(_) => " readable".to_owned(),
                Err(e) => format!(" UNREADABLE: {e}"),
            }
        };
        out.push(format!("  {name}: {size}{readable}"));
        out.push(format!("    {}", describe_access(&path)));
    }
}

#[cfg(target_os = "windows")]
fn describe_access(path: &Path) -> String {
    crate::windows_acl::describe(path).unwrap_or_else(|e| format!("acl: <{e}>"))
}

#[cfg(unix)]
fn describe_access(path: &Path) -> String {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).map_or_else(
        |e| format!("mode: <{e}>"),
        |m| {
            format!(
                "mode {:o} uid {} gid {}",
                m.mode() & 0o7777,
                m.uid(),
                m.gid()
            )
        },
    )
}

#[cfg(not(any(unix, target_os = "windows")))]
fn describe_access(_path: &Path) -> String {
    String::new()
}
