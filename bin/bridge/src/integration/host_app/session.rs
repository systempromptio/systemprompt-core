//! Starting an agent session from a notification's prompt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::HostApp;

/// How a host opens a session seeded with a prompt: a command the bridge runs,
/// or a URL (an app's own deep-link scheme) it hands to the OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartSession {
    Command(Vec<String>),
    Url(String),
}

#[must_use]
pub fn session_starter() -> Option<&'static dyn HostApp> {
    crate::integration::registry::host_apps()
        .iter()
        .copied()
        .find(|host| host.start_session("").is_some())
}
