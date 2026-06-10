//! Process-environment snapshot for the CLI.
//!
//! [`EnvOverrides`] captures every environment variable the CLI consults, read
//! once at process start ([`EnvOverrides::from_process_env`]) and threaded
//! through [`crate::context::CommandContext`]. Command code never calls
//! `std::env::var` directly — tests construct the snapshot with
//! [`EnvOverrides::from_iter`] instead of mutating process state.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct EnvOverrides {
    pub output_format: Option<String>,
    pub log_level: Option<String>,
    pub no_color: bool,
    pub non_interactive: bool,
    pub profile: Option<String>,
    pub rust_log: Option<String>,
    pub is_fly: bool,
    pub is_remote_cli: bool,
    pub editor: Option<String>,
    pub database_url: Option<String>,
    pub services_path: Option<String>,
    pub session: SessionEnv,
}

#[derive(Debug, Clone, Default)]
pub struct SessionEnv {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub context_id: Option<String>,
    pub auth_token: Option<String>,
}

impl EnvOverrides {
    #[must_use]
    pub fn from_process_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    #[must_use]
    pub fn from_iter<I, K, V>(vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let map: HashMap<String, String> = vars
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        Self::from_lookup(|key| map.get(key).cloned())
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            output_format: lookup("SYSTEMPROMPT_OUTPUT_FORMAT"),
            log_level: lookup("SYSTEMPROMPT_LOG_LEVEL"),
            no_color: lookup("SYSTEMPROMPT_NO_COLOR").is_some() || lookup("NO_COLOR").is_some(),
            non_interactive: lookup("SYSTEMPROMPT_NON_INTERACTIVE").is_some(),
            profile: lookup("SYSTEMPROMPT_PROFILE"),
            rust_log: lookup("RUST_LOG"),
            is_fly: lookup("FLY_APP_NAME").is_some(),
            is_remote_cli: lookup("SYSTEMPROMPT_CLI_REMOTE").is_some(),
            editor: lookup("VISUAL").or_else(|| lookup("EDITOR")),
            database_url: lookup("DATABASE_URL"),
            services_path: lookup("SYSTEMPROMPT_SERVICES_PATH"),
            session: SessionEnv {
                user_id: lookup("SYSTEMPROMPT_USER_ID"),
                session_id: lookup("SYSTEMPROMPT_SESSION_ID"),
                context_id: lookup("SYSTEMPROMPT_CONTEXT_ID"),
                auth_token: lookup("SYSTEMPROMPT_AUTH_TOKEN"),
            },
        }
    }
}
