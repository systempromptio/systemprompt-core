//! Unix: a command is started as given, and a local server is stopped as the
//! direct child it is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::Platform;

pub(super) struct Unix;

impl Platform for Unix {
    fn resolve_program(
        command: &str,
        _env: &BTreeMap<String, String>,
        _cwd: Option<&Path>,
    ) -> PathBuf {
        PathBuf::from(command)
    }

    fn end_tree(_pid: u32) {}
}
