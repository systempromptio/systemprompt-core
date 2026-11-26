//! `CliSessionContext` binding a CLI invocation to a persisted session row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_cloud::CliSession;
use systemprompt_extension::cli::CallerSession;
use systemprompt_identifiers::{ContextId, SessionToken};
use systemprompt_manifest::Profile;

#[derive(Debug)]
pub struct CliSessionContext {
    pub session: CliSession,
    pub profile: Profile,
}

impl CliSessionContext {
    pub const fn session_token(&self) -> &SessionToken {
        &self.session.session_token
    }

    pub const fn context_id(&self) -> Option<&ContextId> {
        self.session.context_id.as_ref()
    }

    pub fn api_url(&self) -> &str {
        &self.profile.server.api_external_url
    }

    pub fn caller_session(&self) -> CallerSession {
        CallerSession::new(
            self.session.session_token.clone(),
            self.session.session_id.clone(),
            self.session.user_id.clone(),
            self.session.context_id.clone(),
            self.session.user_type,
        )
    }
}
