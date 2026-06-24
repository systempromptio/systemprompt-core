//! Federated-identity resolution for inbound chat-platform messages.
//!
//! A verified Slack/Teams sender is mapped to a governed systemprompt identity
//! through the same `federated_identities` first-touch path RFC 8693
//! token-exchange uses: a `(issuer, external_sub)` pair resolves to an existing
//! user or mints one on first contact. The platform issuer
//! (`https://slack.com` / the Teams Entra issuer) namespaces the external id so
//! a Slack user and a Teams user with a colliding raw id never alias.

use systemprompt_runtime::AppContext;
use systemprompt_traits::FederatedIdentityClaims;
use systemprompt_users::{User, UserRepository};

use super::MessagingError;

/// Resolve the platform sender to a governed user, linking on first contact.
///
/// The chat platform has already verified the request signature/token, so the
/// `external_user_id` is trusted; no upstream email is asserted, so the minted
/// account carries a synthetic local email and the default `user` role until an
/// operator grants more.
pub async fn resolve_or_link_user(
    ctx: &AppContext,
    issuer: &str,
    external_user_id: &str,
) -> Result<User, MessagingError> {
    let repo =
        UserRepository::new(ctx.db_pool()).map_err(|e| MessagingError::Identity(e.to_string()))?;
    let claims = FederatedIdentityClaims {
        email: None,
        email_verified: false,
        name: None,
        preferred_username: None,
        roles: Vec::new(),
    };
    repo.find_or_create_federated(issuer, external_user_id, &claims)
        .await
        .map_err(|e| MessagingError::Identity(e.to_string()))
}
