//! The run's gateway session.
//!
//! A `user_sessions` row minted in-process, the same row
//! `POST /api/public/gateway/sessions` mints for a PAT caller, and bound to
//! the run's PAT in `api_key_sessions`. The gateway resolves a PAT request to
//! its bound session, so every `/v1/messages` call the run makes lands on it
//! whatever `x-session-id` the harness sends (`OpenCode` 2 replaces the header
//! with its own id). Ended when the run ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ApiKeyId, SessionId, SessionSource, UserId};
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_traits::SessionAnalytics;
use systemprompt_users::{SessionRepository, UserRepository};

use crate::error::{AgentRunsError, AgentRunsResult};

pub(super) async fn open_session(
    creator: &SessionCreationService,
    user_id: &UserId,
    ttl: chrono::Duration,
) -> AgentRunsResult<SessionId> {
    creator
        .create_authenticated_session_with_ttl(
            user_id,
            &SessionAnalytics::default(),
            SessionSource::Api,
            ttl,
        )
        .await
        .map_err(|source| AgentRunsError::Session { source })
}

pub(super) async fn bind_session(
    users: &UserRepository,
    api_key_id: &ApiKeyId,
    session_id: &SessionId,
) -> AgentRunsResult<()> {
    users
        .set_api_key_session(api_key_id, session_id)
        .await
        .map_err(|source| AgentRunsError::UserStore {
            step: "bind session",
            source,
        })
}

pub(super) async fn end_session(
    sessions: &SessionRepository,
    session_id: &SessionId,
) -> AgentRunsResult<()> {
    sessions
        .revoke_session(session_id)
        .await
        .map_err(|source| AgentRunsError::UserStore {
            step: "end session",
            source,
        })
}
