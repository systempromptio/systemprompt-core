//! The command context as the host a contributed command runs against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Context;
use systemprompt_extension::cli::{
    BoxFuture, ChildSupervisor, CliConfig, CommandError, CommandHost, Prompter,
};
use systemprompt_extension::{Admitted, Resolved};
use systemprompt_manifest::Profile;
use systemprompt_runtime::Kernel;
use systemprompt_traits::DatabaseHandle;

use crate::context::CommandContext;

impl CommandContext {
    #[must_use]
    pub fn with_admitted(mut self, admitted: Option<Arc<Admitted>>) -> Self {
        self.admitted = admitted;
        self
    }

    #[must_use]
    pub fn with_kernel(mut self, kernel: Arc<Kernel>) -> Self {
        self.kernel = Some(kernel);
        self
    }

    pub fn admitted(&self) -> anyhow::Result<&Arc<Admitted>> {
        self.admitted
            .as_ref()
            .context("This command requires the composition admitted against a profile")
    }
}

impl CommandHost for CommandContext {
    fn output(&self) -> &CliConfig {
        &self.cli
    }

    fn prompter(&self) -> &dyn Prompter {
        CommandContext::prompter(self)
    }

    fn is_database_scoped(&self) -> bool {
        CommandContext::is_database_scoped(self)
    }

    fn profile(&self) -> Result<&Profile, CommandError> {
        CommandContext::profile(self).map_err(CommandError::new)
    }

    fn db(&self) -> BoxFuture<'_, Result<Arc<dyn DatabaseHandle>, CommandError>> {
        Box::pin(async move {
            let pool = self.db_pool().await.map_err(CommandError::new)?;
            let handle: Arc<dyn DatabaseHandle> = pool;
            Ok(handle)
        })
    }

    fn resolved(&self) -> BoxFuture<'_, Result<Resolved<'_>, CommandError>> {
        Box::pin(async move {
            self.kernel
                .as_ref()
                .map(|kernel| kernel.resolved())
                .ok_or_else(|| {
                    CommandError::new(anyhow::anyhow!(
                        "This command requires the runtime kernel; it runs only with a profile"
                    ))
                })
        })
    }

    fn children(&self) -> &dyn ChildSupervisor {
        &self.foreground_children
    }
}
