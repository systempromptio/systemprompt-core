//! The one implementation of the kernel access a resolution reads through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::Any;
use std::sync::Arc;

use systemprompt_extension::HandleId;
use systemprompt_extension::contribute::KernelAccess;
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

use crate::kernel::Kernel;

impl KernelAccess for Kernel {
    fn database(&self) -> Arc<dyn DatabaseHandle> {
        Arc::clone(&self.database)
    }

    fn tasks(&self) -> &BackgroundTasks {
        &self.tasks
    }

    fn handle(&self, id: HandleId) -> Option<Arc<dyn Any + Send + Sync>> {
        self.handles.get(&id).cloned()
    }
}
