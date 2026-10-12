//! The run's gateway session.
//!
//! A `user_sessions` row minted in-process, the same row
//! `POST /api/public/gateway/sessions` mints for a PAT caller, so every
//! `/v1/messages` call the run makes carries it as `x-session-id`; ended when
//! the run ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_traits::SessionAnalytics;
use systemprompt_users::SessionRepository;

use crate::error::{HeadlessError, HeadlessResult};

pub(super) async fn open_session(
    creator: &SessionCreationService,
    user_id: &UserId,
    ttl: chrono::Duration,
) -> HeadlessResult<SessionId> {
    creator
        .create_authenticated_session_with_ttl(
            user_id,
            &SessionAnalytics::default(),
            SessionSource::Api,
            ttl,
        )
        .await
        .map_err(|source| HeadlessError::Session { source })
}

pub(super) async fn end_session(
    sessions: &SessionRepository,
    session_id: &SessionId,
) -> HeadlessResult<()> {
    sessions
        .revoke_session(session_id)
        .await
        .map_err(|source| HeadlessError::UserStore {
            step: "end session",
            source,
        })
}
