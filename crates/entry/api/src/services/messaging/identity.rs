//! Federated-identity resolution for inbound chat-platform messages.
//!
//! A verified Slack/Teams sender is mapped to a governed systemprompt identity
//! through the one federated resolution SSO and RFC 8693 token exchange use:
//! a `(issuer, external_sub)` pair resolves to an existing user, links an
//! ordinary account by verified email only when the app's link policy allows
//! it, or mints one on first contact. The platform issuer
//! (`https://slack.com` / the Teams Entra issuer) namespaces the external id so
//! a Slack user and a Teams user with a colliding raw id never alias.
//!
//! The claims are supplied by the platform route, which is the only layer that
//! can read the sender's profile. A route that reads nothing passes empty
//! claims and the sender lands on a fresh, role-less user — never on an
//! existing account.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_runtime::AppContext;
use systemprompt_traits::{FederatedIdentityClaims, LinkPolicy};
use systemprompt_users::User;

use super::MessagingError;

pub async fn resolve_or_link_user(
    ctx: &AppContext,
    issuer: &str,
    external_user_id: &str,
    claims: &FederatedIdentityClaims,
    policy: &LinkPolicy,
) -> Result<User, MessagingError> {
    let repo = ctx.user_repository();
    repo.resolve_federated(issuer, external_user_id, claims, policy)
        .await
        .map(|(user, _link)| user)
        .map_err(MessagingError::Identity)
}
