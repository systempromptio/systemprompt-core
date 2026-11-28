//! Unix: paths stand as written, a command runs when a file carries an
//! executable bit, and a server is stopped as the direct child it is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use tokio::process::Child;

use super::Platform;
use crate::lsp::uri::PathStyle;

pub(super) struct Unix;

impl Platform for Unix {
    const PATH_STYLE: PathStyle = PathStyle::Posix;

    fn settle(path: PathBuf) -> PathBuf {
        path
    }

    fn runnable(candidate: &Path, _extensions: &[String]) -> Option<PathBuf> {
        std::fs::metadata(candidate)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            .then(|| candidate.to_path_buf())
    }

    async fn end_tree(_child: &Child) {}

    fn abandon_tree(_child: &mut Child) {}
}
