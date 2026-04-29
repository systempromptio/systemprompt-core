#![cfg(feature = "dev-stub-host")]

use std::collections::BTreeMap;

use crate::integration::host_app::{
    GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema, ProfileGenInputs, ProfileState,
};

pub struct StubHost;

pub static STUB_HOST: StubHost = StubHost;

static SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: &[],
    display_keys: &[],
};

impl HostApp for StubHost {
    fn id(&self) -> &'static str {
        "stub"
    }

    fn display_name(&self) -> &'static str {
        "Example Host (dev stub)"
    }

    fn config_schema(&self) -> &'static HostConfigSchema {
        &SCHEMA
    }

    fn probe(&self) -> HostAppSnapshot {
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state: ProfileState::Absent,
            profile_source: None,
            profile_keys: BTreeMap::new(),
            host_running: false,
            host_processes: Vec::new(),
            probed_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    fn generate_profile(&self, _inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "stub host: generate not implemented",
        ))
    }

    fn install_profile(&self, _path: &str) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "stub host: install not implemented",
        ))
    }

    fn install_action_label(&self) -> &'static str {
        "no-op"
    }
}
