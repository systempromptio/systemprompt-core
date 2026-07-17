//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::env_overrides::EnvOverrides;

#[derive(Debug, Clone, Copy)]
pub struct ExecutionEnvironment {
    pub is_fly: bool,
    pub is_remote_cli: bool,
}

impl ExecutionEnvironment {
    #[must_use]
    pub const fn from_env(env: &EnvOverrides) -> Self {
        Self {
            is_fly: env.is_fly,
            is_remote_cli: env.is_remote_cli,
        }
    }
}
