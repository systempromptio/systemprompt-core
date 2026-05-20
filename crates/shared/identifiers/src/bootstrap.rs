//! Sanctioned producers of compile-time `UserId` values.
//!
//! Every identity below corresponds to a non-user actor that the platform
//! must attribute work to: pre-auth requests, crawler traffic, internal
//! fallbacks, and schema-level sentinels. Adding a new helper here is a
//! security-review-class change — the allowlist is small on purpose, and the
//! `&'static str` bound on `UserId::bootstrap` makes it impossible to feed
//! dynamic data through this module.
//!
//! Runtime-sourced strings (DB rows, HTTP headers, config values) must use
//! [`UserId::new`] directly, never the helpers here.

use crate::UserId;

pub fn admin() -> UserId {
    UserId::bootstrap("admin")
}

pub fn anonymous() -> UserId {
    UserId::bootstrap("anonymous")
}

pub fn unknown() -> UserId {
    UserId::bootstrap("unknown")
}

pub fn bot() -> UserId {
    UserId::bootstrap("bot")
}

pub fn default() -> UserId {
    UserId::bootstrap("default")
}

pub fn empty_sentinel() -> UserId {
    UserId::bootstrap("")
}
