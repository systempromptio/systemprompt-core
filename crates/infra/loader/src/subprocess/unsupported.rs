//! Fail-closed child-supervision stubs for platforms with no way to read
//! another process's environment.
//!
//! Nothing here ever confirms an identity, so no process is signalled on a
//! guess; orphaned children must be stopped by hand.
//! [`identity_verification_supported`](systemprompt_models::subprocess::identity_verification_supported)
//! reports `false` on these platforms so
//! callers can say so rather than blaming a foreign owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[must_use]
pub const fn is_zombie(_pid: u32) -> bool {
    false
}

pub(super) fn live_environ(pid: u32) -> Option<Vec<u8>> {
    tracing::warn!(
        pid,
        "Child identity cannot be verified on this platform, so this process will not be \
         signalled; it must be stopped by hand"
    );
    None
}
