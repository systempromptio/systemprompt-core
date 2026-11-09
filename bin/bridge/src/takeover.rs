//! A second launch while a bridge is already running: whether the new process
//! takes over, and the loopback control requests the running GUI answers.
//!
//! A launch of a newer build asks the running instance to hand over, so an
//! update installed on disk runs as soon as the new binary is opened. A launch
//! of the same or an older build focuses the running window instead. A running
//! instance whose sidecar carries no version predates the handoff request; it
//! counts as older, and because it does not answer the request the launcher
//! falls back to focusing it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::release_version::is_newer;

pub const FOCUS_PATH: &str = "/api/focus_window";
pub const HANDOFF_PATH: &str = "/api/handoff";

/// What a launch does when another bridge already holds the singleton lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondLaunch {
    TakeOver,
    Focus,
}

#[must_use]
pub fn second_launch(this: &str, running: Option<&str>) -> SecondLaunch {
    match running {
        Some(running) if !is_newer(this, running) => SecondLaunch::Focus,
        _ => SecondLaunch::TakeOver,
    }
}

/// A request the running GUI's loopback control server accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRequest {
    Focus,
    Handoff,
}

#[must_use]
pub fn parse_control_request(head: &str, csrf: &str) -> Option<ControlRequest> {
    let request_line = head.lines().next()?;
    let mut parts = request_line.split_whitespace();
    if parts.next()? != "POST" {
        return None;
    }
    let path_with_query = parts.next()?;
    let (path, query) = path_with_query
        .split_once('?')
        .unwrap_or((path_with_query, ""));
    let supplied = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("t="))
        .unwrap_or("");
    if supplied.is_empty() || !crate::hash::constant_time_eq(supplied.as_bytes(), csrf.as_bytes()) {
        return None;
    }
    match path {
        FOCUS_PATH => Some(ControlRequest::Focus),
        HANDOFF_PATH => Some(ControlRequest::Handoff),
        _ => None,
    }
}

#[must_use]
pub fn control_request(path: &str, token: &str) -> String {
    format!(
        "POST {path}?t={token} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: \
         close\r\n\r\n"
    )
}
