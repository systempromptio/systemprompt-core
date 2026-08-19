//! Windows post-install bootstrap steps.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(not(unix))]

use std::path::Path;

pub(super) const fn chown_to_sudo_user_if_root(_path: &Path) {}
