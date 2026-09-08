//! Choosing and recording the loopback port: probe each candidate for a
//! sibling, bind the first free one, remember it for the other processes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use tokio::runtime::Handle;

use super::identity::InstallId;
use super::peer::{self, PeerIdentity};
use super::{DEFAULT_PROXY_PORT, MAX_CANDIDATE_PORT, candidate_ports, portfile, server};
use crate::obs::StartupFault;
use crate::stdio::diag;

pub(super) enum Bind {
    Listener(tokio::net::TcpListener),
    Sibling {
        port: u16,
        pid: u32,
        config_dir: String,
    },
    Exhausted,
}

pub(super) fn bind_candidate(
    rt: &Handle,
    ours: &InstallId,
    tried: &mut Vec<u16>,
    last_error: &mut String,
    faults: &mut Vec<StartupFault>,
) -> Bind {
    let ports = match candidate_ports(ours) {
        Ok(ports) => ports,
        Err(e) => {
            faults.push(StartupFault::new("proxy port file", e));
            super::candidate_ports_from(None)
        },
    };
    for port in ports {
        if port != 0 {
            match peer::probe_identity(port, ours) {
                PeerIdentity::Ours(who) => {
                    return Bind::Sibling {
                        port: who.port,
                        pid: who.pid,
                        config_dir: who.config_dir,
                    };
                },
                PeerIdentity::Foreign(who) => {
                    diag(&format!(
                        "proxy: port {port} is held by another {} install ({}); trying the next \
                         port",
                        crate::brand::brand().app_name,
                        who.config_dir
                    ));
                    tried.push(port);
                    continue;
                },
                PeerIdentity::Unknown => {
                    diag(&format!(
                        "proxy: port {port} is held by an unidentified listener; trying the next \
                         port"
                    ));
                    tried.push(port);
                    continue;
                },
                PeerIdentity::Unreachable => {},
            }
        }

        match rt.block_on(server::try_bind(port)) {
            Ok(l) => return Bind::Listener(l),
            Err(e) => {
                *last_error = e.to_string();
                tried.push(port);
            },
        }
    }
    Bind::Exhausted
}

pub(super) fn persist_and_announce(port: u16, ours: &InstallId) -> std::io::Result<()> {
    if !(DEFAULT_PROXY_PORT..=MAX_CANDIDATE_PORT).contains(&port) {
        return Err(std::io::Error::other(
            "cannot publish an unstable proxy port",
        ));
    }
    portfile::write(port, ours)?;

    if port == DEFAULT_PROXY_PORT {
        diag(&format!("proxy: listening on localhost:{port}"));
        return Ok(());
    }

    let bin = crate::brand::brand().binary_name;
    diag(&format!(
        "proxy: port {DEFAULT_PROXY_PORT} was taken by another listener; listening on {port} \
         instead.\n       Client configs written for port {DEFAULT_PROXY_PORT} will be rejected \
         with 403 — run `{bin} install --apply` to repoint them, then restart the client."
    ));
    Ok(())
}

pub(super) fn portfile_port(ours: &InstallId) -> std::io::Result<Option<u16>> {
    let Some(record) = portfile::read(ours)? else {
        return Ok(None);
    };
    match peer::probe_identity(record.port, ours) {
        PeerIdentity::Unknown => Err(std::io::Error::other(format!(
            "recorded proxy port {} belongs to an unidentified listener",
            record.port
        ))),
        PeerIdentity::Ours(_) | PeerIdentity::Unreachable => Ok(Some(record.port)),
        PeerIdentity::Foreign(who) => {
            tracing::warn!(
                port = record.port,
                other = %who.config_dir,
                "our recorded proxy port is now held by another install",
            );
            Ok(None)
        },
    }
}
