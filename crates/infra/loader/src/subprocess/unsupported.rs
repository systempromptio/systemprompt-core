//! Fail-closed child-supervision stubs for platforms with no way to read
//! another process's environment.
//!
//! Nothing here ever confirms an identity, so no process is signalled on a
//! guess; orphaned children must be stopped by hand.
//! `identity_verification_supported` reports `false` on these platforms so
//! callers can say so rather than blaming a foreign owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[must_use]
pub fn live_pid_is_subprocess(pid: u32, _name_key: &str, service_name: &str) -> bool {
    tracing::warn!(
        pid,
        service = %service_name,
        "Child identity cannot be verified on this platform, so this process will not be \
         signalled; it must be stopped by hand"
    );
    false
}

#[must_use]
pub const fn is_zombie(_pid: u32) -> bool {
    false
}
