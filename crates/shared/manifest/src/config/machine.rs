//! The identity of the machine a process runs on, for the local-profile
//! instance id.
//!
//! Every process of one local profile on one machine — the server, the
//! services it spawns and a CLI run — must resolve the same instance id, and a
//! process on another machine never may: the service registry, the instance
//! claim and node-scoped scheduler runs are keyed by it. The identity is read
//! from the operating system (the systemd/D-Bus machine id, else the kernel
//! host name), never from the environment, and only a SHA-256 prefix of it is
//! used, so the raw machine id is not published in logs or rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use sha2::{Digest, Sha256};

const MACHINE_ID_FILES: [&str; 2] = ["/etc/machine-id", "/var/lib/dbus/machine-id"];
const FINGERPRINT_HEX: usize = 12;

/// A short, stable fingerprint of the machine this process runs on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MachineIdentity(String);

impl MachineIdentity {
    #[must_use]
    pub fn probe() -> Option<Self> {
        MACHINE_ID_FILES
            .iter()
            .find_map(|path| std::fs::read_to_string(path).ok())
            .and_then(|raw| Self::from_source(&raw))
            .or_else(|| host_name().and_then(|name| Self::from_source(&name)))
    }

    #[must_use]
    pub fn from_source(source: &str) -> Option<Self> {
        let source = source.trim();
        if source.is_empty() {
            return None;
        }
        let digest = Sha256::digest(source.as_bytes());
        let hex = hex::encode(digest);
        Some(Self(hex[..FINGERPRINT_HEX].to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MachineIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(unix)]
fn host_name() -> Option<String> {
    nix::unistd::gethostname()
        .ok()
        .and_then(|name| name.into_string().ok())
}

#[cfg(not(unix))]
const fn host_name() -> Option<String> {
    None
}
