//! Starting and stopping the server process without a console window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::process::Stdio;

use tokio::process::{Child, Command};
use tracing::trace;

use crate::lsp::platform;
use crate::lsp::spec::ServerSpec;

pub(super) fn server_command(program: &Path, spec: &ServerSpec, root: &Path) -> Command {
    let mut command = platform::command(program);
    command
        .args(&spec.args)
        .envs(&spec.env)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

pub(super) async fn kill(child: &mut Child) {
    platform::end_tree(child).await;
    if let Err(error) = child.kill().await {
        trace!(%error, "language server already gone");
    }
}
