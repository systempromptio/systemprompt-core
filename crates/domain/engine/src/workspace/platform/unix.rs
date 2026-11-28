//! Unix: keys are compared as written, a key segment is a plain name, a
//! command shim goes by the command's own name, and write and execute
//! permissions are mode bits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::{fs, io};

use super::Platform;

pub(super) struct Unix;

impl Platform for Unix {
    fn fold(value: &str) -> String {
        value.to_owned()
    }

    fn leaves_directory(_segment: &str) -> bool {
        false
    }

    fn separators() -> &'static [char] {
        &['/']
    }

    fn shim_names(command: &str) -> Vec<String> {
        vec![command.to_owned()]
    }

    fn make_writable(path: &Path, permissions: &fs::Permissions) -> io::Result<()> {
        fs::set_permissions(path, fs::Permissions::from_mode(permissions.mode() | 0o200))
    }

    fn set_executable(path: &Path, executable: bool) -> io::Result<()> {
        let mode = fs::metadata(path)?.permissions().mode();
        let wanted = if executable {
            mode | ((mode & 0o444) >> 2)
        } else {
            mode & !0o111
        };
        if wanted != mode {
            fs::set_permissions(path, fs::Permissions::from_mode(wanted))?;
        }
        Ok(())
    }

    fn is_executable(metadata: &fs::Metadata) -> bool {
        metadata.permissions().mode() & 0o111 != 0
    }
}
