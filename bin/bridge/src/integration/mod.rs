pub(crate) mod app_launch;
pub mod claude_code_cli;
pub mod claude_desktop;
pub mod codex_cli;
pub mod cowork_plugins;
pub mod host_app;
pub mod proxy_probe;
pub mod registry;
#[cfg(feature = "dev-stub-host")]
pub mod stub_host;

pub use host_app::{
    ConfigFormat, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema, ProfileGenInputs,
    ProfileState,
};
pub use proxy_probe::{ProxyHealth, ProxyProbeState};
pub use registry::{find_host_by_id, host_apps};
