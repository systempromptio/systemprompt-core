use std::sync::LazyLock;

use super::host_app::HostApp;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use super::claude_desktop::CLAUDE_DESKTOP_HOST;
use super::codex_cli::CODEX_CLI_HOST;
#[cfg(feature = "dev-stub-host")]
use super::stub_host::STUB_HOST;

#[cfg(any(target_os = "macos", target_os = "windows"))]
const DESKTOP_HOSTS: &[&'static dyn HostApp] = &[&CLAUDE_DESKTOP_HOST];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DESKTOP_HOSTS: &[&'static dyn HostApp] = &[];

#[cfg(feature = "dev-stub-host")]
const STUB_HOSTS: &[&'static dyn HostApp] = &[&STUB_HOST];
#[cfg(not(feature = "dev-stub-host"))]
const STUB_HOSTS: &[&'static dyn HostApp] = &[];

static REGISTRY: LazyLock<Vec<&'static dyn HostApp>> = LazyLock::new(|| {
    DESKTOP_HOSTS
        .iter()
        .copied()
        .chain(std::iter::once(&CODEX_CLI_HOST as &'static dyn HostApp))
        .chain(STUB_HOSTS.iter().copied())
        .collect()
});

pub fn host_apps() -> &'static [&'static dyn HostApp] {
    REGISTRY.as_slice()
}

#[must_use]
pub fn find_host_by_id(id: &str) -> Option<&'static dyn HostApp> {
    REGISTRY.iter().copied().find(|h| h.id() == id)
}
