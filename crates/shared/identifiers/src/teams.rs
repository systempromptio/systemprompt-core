//! Typed identifiers for the Microsoft Teams integration — the Entra (Azure AD)
//! tenant, the Bot Framework conversation, and the end-user (AAD object id)
//! identifiers that Teams assigns. These are opaque Microsoft-side strings; the
//! integration never mints them, only carries them through dispatch and the
//! federated-identity mapping.

crate::define_id!(TeamsTenantId);
crate::define_id!(TeamsConversationId);
crate::define_id!(TeamsUserId);
