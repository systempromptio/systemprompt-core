//! A person's UI session: the bearer token a first-party client the operator
//! declared `person_ui` holds for the person who signed in through it.
//!
//! Only the authorization-code + PKCE grant for such a client mints one, and
//! only refreshing that grant's own refresh token renews it. The token carries
//! the `ui_session` claim and exactly the [`JwtAudience::Ui`] audience, a
//! `client_id`, no `purpose` and no `act` chain
//! ([`JwtClaims::ui_session_well_formed`]); the access token lives
//! [`UI_SESSION_ACCESS_TTL_SECS`], and its `auth_time` is the sign-in, carried
//! unchanged through every refresh until [`UI_SESSION_ABSOLUTE_SECS`] after it.
//! A high-impact document action wants a sign-in no older than
//! [`UI_SESSION_STEP_UP_MAX_AGE_SECS`] (RFC 9470).
//!
//! [`JwtAudience::Ui`]: super::JwtAudience::Ui
//! [`JwtClaims::ui_session_well_formed`]: super::JwtClaims::ui_session_well_formed
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const UI_SESSION_ACCESS_TTL_SECS: i64 = 300;

pub const UI_SESSION_ABSOLUTE_SECS: i64 = 12 * 60 * 60;

pub const UI_SESSION_STEP_UP_MAX_AGE_SECS: i64 = 600;

#[must_use]
pub const fn step_up_required(auth_time: i64, now: i64) -> bool {
    now.saturating_sub(auth_time) > UI_SESSION_STEP_UP_MAX_AGE_SECS
}
