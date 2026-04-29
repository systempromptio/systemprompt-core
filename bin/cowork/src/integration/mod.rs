pub mod claude_desktop;
pub mod host_app;
pub mod proxy_probe;
pub mod registry;
#[cfg(feature = "dev-stub-host")]
pub mod stub_host;

pub use host_app::{
    GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema, ProfileGenInputs, ProfileState,
};
pub use proxy_probe::{ProxyHealth, ProxyProbeState};
pub use registry::{host_apps, host_by_id};
