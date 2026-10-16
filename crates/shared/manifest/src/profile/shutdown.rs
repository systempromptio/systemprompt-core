//! `server.shutdown`: the deadlines a node honours between the termination
//! signal and process exit.
//!
//! The sequence is fixed; only its windows are configurable:
//!
//! 1. `/readyz` flips to `draining` the moment the signal arrives, and the
//!    listener keeps accepting for `pre_stop_delay_secs` so an orchestrator's
//!    endpoint controller stops routing to the node first.
//! 2. The listener stops accepting and in-flight requests — long SSE streams
//!    included — get the full `drain_grace_secs` to finish.
//! 3. Child processes get `child_grace_secs` after their signal and owned
//!    background work (the final gateway journal settlement included)
//!    `background_grace_secs`, in parallel.
//! 4. `forced_exit_grace_secs`, armed once step 2 ends, bounds step 3 and the
//!    log flush; when it fires the process exits non-zero.
//!
//! An orchestrator's termination grace (`terminationGracePeriodSeconds`) must
//! cover [`ShutdownConfig::total_budget`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const MAX_PRE_STOP_DELAY_SECS: u64 = 600;
pub const MAX_DRAIN_GRACE_SECS: u64 = 86_400;

/// Shutdown deadlines in seconds. The defaults are the historical fixed
/// windows: no pre-stop delay, a 10 s drain, 5 s for children and background
/// work, and a 10 s forced-exit backstop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "every field is a window in seconds; the unit belongs in the name"
)]
pub struct ShutdownConfig {
    #[serde(default)]
    pub pre_stop_delay_secs: u64,
    #[serde(default = "default_drain_grace_secs")]
    pub drain_grace_secs: u64,
    #[serde(default = "default_child_grace_secs")]
    pub child_grace_secs: u64,
    #[serde(default = "default_background_grace_secs")]
    pub background_grace_secs: u64,
    #[serde(default = "default_forced_exit_grace_secs")]
    pub forced_exit_grace_secs: u64,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            pre_stop_delay_secs: 0,
            drain_grace_secs: default_drain_grace_secs(),
            child_grace_secs: default_child_grace_secs(),
            background_grace_secs: default_background_grace_secs(),
            forced_exit_grace_secs: default_forced_exit_grace_secs(),
        }
    }
}

impl ShutdownConfig {
    #[must_use]
    pub const fn pre_stop_delay(&self) -> Duration {
        Duration::from_secs(self.pre_stop_delay_secs)
    }

    #[must_use]
    pub const fn drain_grace(&self) -> Duration {
        Duration::from_secs(self.drain_grace_secs)
    }

    #[must_use]
    pub const fn child_grace(&self) -> Duration {
        Duration::from_secs(self.child_grace_secs)
    }

    #[must_use]
    pub const fn background_grace(&self) -> Duration {
        Duration::from_secs(self.background_grace_secs)
    }

    #[must_use]
    pub const fn forced_exit_grace(&self) -> Duration {
        Duration::from_secs(self.forced_exit_grace_secs)
    }

    #[must_use]
    pub const fn total_budget(&self) -> Duration {
        Duration::from_secs(
            self.pre_stop_delay_secs
                .saturating_add(self.drain_grace_secs)
                .saturating_add(self.forced_exit_grace_secs),
        )
    }

    #[must_use]
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.pre_stop_delay_secs > MAX_PRE_STOP_DELAY_SECS {
            errors.push(format!(
                "server.shutdown.pre_stop_delay_secs must be at most {MAX_PRE_STOP_DELAY_SECS} \
                 (got {})",
                self.pre_stop_delay_secs
            ));
        }
        if self.drain_grace_secs == 0 || self.drain_grace_secs > MAX_DRAIN_GRACE_SECS {
            errors.push(format!(
                "server.shutdown.drain_grace_secs must be between 1 and {MAX_DRAIN_GRACE_SECS} \
                 (got {})",
                self.drain_grace_secs
            ));
        }
        if self.child_grace_secs == 0 {
            errors.push("server.shutdown.child_grace_secs must be greater than 0".to_owned());
        }
        if self.background_grace_secs == 0 {
            errors.push("server.shutdown.background_grace_secs must be greater than 0".to_owned());
        }
        let teardown = self.child_grace_secs.max(self.background_grace_secs);
        if self.forced_exit_grace_secs <= teardown {
            errors.push(format!(
                "server.shutdown.forced_exit_grace_secs ({}) must exceed both child_grace_secs \
                 and background_grace_secs ({teardown}); the backstop would kill the teardown it \
                 bounds",
                self.forced_exit_grace_secs
            ));
        }
        errors
    }
}

const fn default_drain_grace_secs() -> u64 {
    10
}

const fn default_child_grace_secs() -> u64 {
    5
}

const fn default_background_grace_secs() -> u64 {
    5
}

const fn default_forced_exit_grace_secs() -> u64 {
    10
}
