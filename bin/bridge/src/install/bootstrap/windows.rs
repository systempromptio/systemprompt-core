#![cfg(not(unix))]

use std::path::Path;

pub(super) fn chown_to_sudo_user_if_root(_path: &Path) {}
