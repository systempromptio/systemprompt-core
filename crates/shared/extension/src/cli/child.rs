//! The foreground child registry a command spawns supervised children
//! through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

/// Foreground child registry, implemented by the loader's
/// foreground-children registry.
pub trait ChildSupervisor: Send + Sync {
    fn handle(&self) -> Arc<dyn ChildSupervisor>;
    fn output(&self, cmd: std::process::Command) -> std::io::Result<std::process::Output>;
    fn status(&self, cmd: std::process::Command) -> std::io::Result<std::process::ExitStatus>;
    fn status_with_stdin(
        &self,
        cmd: std::process::Command,
        stdin: &[u8],
    ) -> std::io::Result<std::process::ExitStatus>;
}
