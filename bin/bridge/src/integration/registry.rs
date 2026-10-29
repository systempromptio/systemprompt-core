//! Compile-time registry of available desktop host integrations.
//!
//! Hosts are contributed through the `inventory` crate: built-ins submit via
//! [`register_host_app!`](crate::register_host_app) below, and white-label
//! crates can supply their own implementation of a host without editing core.
//! Every registration is keyed by its [`HostKind`] — the closed set of hosts
//! the gateway knows — so a host outside that set cannot be registered.
//! Registrations carry a `priority` (built-ins use 0); the registry sorts by
//! descending priority then `id()`, then **dedups by `id()` keeping the
//! highest-priority entry** — so a white-label crate can shadow a built-in
//! host by re-registering its kind at `priority > 0`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use systemprompt_models::bridge::host::HostKind;

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
register_host_app!(super::codex::CODEX_APP_HOST);
register_host_app!(super::codex::CODEX_CLI_HOST);
register_host_app!(super::hermes::HERMES_HOST);
register_host_app!(super::opencode::OPENCODE_APP_HOST);
register_host_app!(super::opencode::OPENCODE_CLI_HOST);

struct Registry {
    hosts: Vec<&'static dyn HostApp>,
}

static REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
    let mut regs: Vec<&'static HostAppRegistration> =
        inventory::iter::<HostAppRegistration>().collect();
    regs.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.app.id().cmp(&b.app.id()))
    });
    let mut seen: BTreeSet<HostKind> = BTreeSet::new();
    let mut hosts: Vec<&'static dyn HostApp> = regs
        .into_iter()
        .filter(|r| seen.insert(r.app.id()))
        .map(|r| r.app)
        .collect();
    hosts.sort_by_key(|h| h.id());
    Registry { hosts }
});

pub fn host_apps() -> &'static [&'static dyn HostApp] {
    REGISTRY.hosts.as_slice()
}

#[must_use]
pub fn find_host_by_id(id: HostKind) -> Option<&'static dyn HostApp> {
    REGISTRY.hosts.iter().copied().find(|h| h.id() == id)
}

/// What a host id means to this binary.
///
/// Every caller acting on a user-supplied host id resolves it through
/// [`resolve_host`] rather than treating a [`find_host_by_id`] miss as an
/// unknown id: a sync-only agent is ordinary, and only
/// [`ResolvedHost::Unknown`] is a caller error.
#[derive(Clone, Copy)]
pub enum ResolvedHost {
    Local(&'static dyn HostApp),
    SyncOnly(&'static super::sync_only::SyncOnlyAgent),
    Unknown,
}

impl std::fmt::Debug for ResolvedHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Local(host) => f.debug_tuple("Local").field(&host.id()).finish(),
            Self::SyncOnly(agent) => f.debug_tuple("SyncOnly").field(&agent.id).finish(),
            Self::Unknown => f.write_str("Unknown"),
        }
    }
}

#[must_use]
pub fn resolve_host(id: HostKind) -> ResolvedHost {
    if let Some(host) = find_host_by_id(id) {
        return ResolvedHost::Local(host);
    }
    if let Some(agent) = super::sync_only::sync_only_agent(id) {
        return ResolvedHost::SyncOnly(agent);
    }
    ResolvedHost::Unknown
}
