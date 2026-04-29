use std::sync::LazyLock;

use super::host_app::HostApp;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use super::claude_desktop::CLAUDE_DESKTOP_HOST;
#[cfg(feature = "dev-stub-host")]
use super::stub_host::STUB_HOST;

static REGISTRY: LazyLock<Vec<&'static dyn HostApp>> = LazyLock::new(|| {
    #[allow(unused_mut)]
    let mut entries: Vec<&'static dyn HostApp> = Vec::new();
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    entries.push(&CLAUDE_DESKTOP_HOST);
    #[cfg(feature = "dev-stub-host")]
    entries.push(&STUB_HOST);
    entries
});

pub fn host_apps() -> &'static [&'static dyn HostApp] {
    REGISTRY.as_slice()
}

pub fn host_by_id(id: &str) -> Option<&'static dyn HostApp> {
    REGISTRY.iter().copied().find(|h| h.id() == id)
}
