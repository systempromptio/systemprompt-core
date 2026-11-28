//! What the store asks of the operating system: the store's database file
//! is created readable and writable by its owner only, where the system has
//! such a mode.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::OpenOptions;

#[cfg(unix)]
pub(super) fn owner_only(options: &mut OpenOptions) {
    std::os::unix::fs::OpenOptionsExt::mode(options, 0o600);
}

// Why: Windows has no mode bits; the file inherits the data root's ACL, which
// the device restricts to the user (systemprompt-device's platform module).
#[cfg(not(unix))]
pub(super) const fn owner_only(_options: &mut OpenOptions) {}
