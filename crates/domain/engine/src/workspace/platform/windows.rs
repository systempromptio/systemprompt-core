//! Windows: keys are compared in lower case, a backslash or a colon in a key
//! segment names another place, a command shim is a `.cmd` or an `.exe`, and
//! a file has one read-only attribute and no executable bit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::{fs, io};

use super::Platform;

pub(super) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(super) struct Windows;

impl Platform for Windows {
    fn fold(value: &str) -> String {
        value.to_lowercase()
    }

    fn leaves_directory(segment: &str) -> bool {
        segment.contains(['\\', ':'])
    }

    fn separators() -> &'static [char] {
        &['/', '\\']
    }

    fn shim_names(command: &str) -> Vec<String> {
        vec![format!("{command}.cmd"), format!("{command}.exe")]
    }

    fn make_writable(path: &Path, permissions: &fs::Permissions) -> io::Result<()> {
        let mut permissions = permissions.clone();
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "Windows has one read-only attribute; clearing it is the only way to rewrite \
                      or delete the file"
        )]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)
    }

    fn set_executable(_path: &Path, _executable: bool) -> io::Result<()> {
        Ok(())
    }

    fn is_executable(_metadata: &fs::Metadata) -> bool {
        false
    }
}
