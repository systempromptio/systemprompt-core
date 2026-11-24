//! The `Debug` rendering of the scheduler service.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SchedulerService;

impl std::fmt::Debug for SchedulerService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchedulerService")
            .field("config", &self.config)
            .field("users", &self.users.is_some())
            .finish_non_exhaustive()
    }
}
