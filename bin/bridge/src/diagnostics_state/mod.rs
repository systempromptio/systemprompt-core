//! The local state a diagnostics bundle must carry to explain a broken proxy.
//!
//! Which process owns the port, what the key files look like, how every host
//! profile and the org-plugins tree are protected, and what the machine policy
//! currently says. `registry` renders the Windows policy hives separately.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod files;
mod hosts;
pub(crate) mod registry;

use std::path::Path;

use self::files::{append_dir, append_file};
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
    hosts::append_org_plugins(&mut out);
    out.push(String::new());
    hosts::append_host_profiles(&mut out, ctx);
    out.push(String::new());
    hosts::append_working_dirs(&mut out);
    out.push(String::new());
    hosts::append_single_instance(&mut out);
    out.push(String::new());
    hosts::append_update(&mut out);
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
