//! The `Debug` rendering of the command context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CommandContext;

impl std::fmt::Debug for CommandContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandContext")
            .field("cli", &self.cli)
            .field("env", &self.env)
            .field("database_scoped", &self.db.is_some())
            .finish_non_exhaustive()
    }
}
