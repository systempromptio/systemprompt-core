//! Compile-time registry of available desktop host integrations.
//!
//! Hosts are contributed through the `inventory` crate: built-ins submit via
//! [`register_host_app!`] below, and white-label crates can register their own
//! without editing core. Registrations carry a `priority` (built-ins use 0);
//! the registry sorts by descending priority then `id()`, then **dedups by
//! `id()` keeping the highest-priority entry** — so a white-label crate can
//! shadow a built-in host by re-registering its id at `priority > 0`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use super::host_app::HostApp;

#[derive(Clone, Copy)]
pub struct HostAppRegistration {
    pub app: &'static dyn HostApp,
    pub priority: i32,
}

impl std::fmt::Debug for HostAppRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostAppRegistration")
            .field("id", &self.app.id())
            .field("priority", &self.priority)
            .finish()
    }
}

inventory::collect!(HostAppRegistration);

/// Register a [`HostApp`] into the compile-time host registry.
///
/// Pass a zero-sized `'static` host value (e.g. a unit-struct instance). An
/// optional `priority = N` (default 0) lets a registration shadow a built-in
/// sharing the same `id()`: the highest priority wins.
#[macro_export]
macro_rules! register_host_app {
    ($e:expr, priority = $p:expr $(,)?) => {
        ::inventory::submit! {
            $crate::integration::registry::HostAppRegistration { app: &$e, priority: $p }
        }
    };
    ($e:expr $(,)?) => {
        $crate::register_host_app!($e, priority = 0);
    };
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
register_host_app!(super::claude_desktop::CLAUDE_DESKTOP_HOST);
register_host_app!(super::codex_cli::CODEX_CLI_HOST);
#[cfg(feature = "dev-stub-host")]
register_host_app!(super::stub_host::STUB_HOST);

static REGISTRY: LazyLock<Vec<&'static dyn HostApp>> = LazyLock::new(|| {
    let mut regs: Vec<&'static HostAppRegistration> =
        inventory::iter::<HostAppRegistration>().collect();
    regs.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.app.id().cmp(b.app.id()))
    });
    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    let mut v: Vec<&'static dyn HostApp> = regs
        .into_iter()
        .filter(|r| seen.insert(r.app.id()))
        .map(|r| r.app)
        .collect();
    v.sort_by_key(|h| h.id());
    v
});

pub fn host_apps() -> &'static [&'static dyn HostApp] {
    REGISTRY.as_slice()
}

#[must_use]
pub fn find_host_by_id(id: &str) -> Option<&'static dyn HostApp> {
    REGISTRY.iter().copied().find(|h| h.id() == id)
}
