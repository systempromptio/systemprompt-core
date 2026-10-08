//! Typed identifiers for the Microsoft Teams integration — the Entra (Azure AD)
//! tenant, the Bot Framework app registration (Microsoft App ID), the Bot
//! Framework conversation, and the end-user (AAD object id) identifiers that
//! Teams assigns. These are opaque Microsoft-side strings; the integration
//! never mints them, only carries them through dispatch and the
//! federated-identity mapping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(TeamsTenantId, checked, |value| {
    crate::macros::validate_non_empty("TeamsTenantId", value)
});
crate::define_id!(TeamsAppId, checked, |value| {
    crate::macros::validate_non_empty("TeamsAppId", value)
});
crate::define_id!(TeamsConversationId, checked, |value| {
    crate::macros::validate_non_empty("TeamsConversationId", value)
});
crate::define_id!(TeamsUserId, checked, |value| {
    crate::macros::validate_non_empty("TeamsUserId", value)
});
