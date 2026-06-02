//! User identifier.
//!
//! Every `UserId` must originate from a row in the `users` table. The
//! request middleware persists an anonymous user before constructing a
//! request context (see `SessionCreationService::ensure_anonymous_user`);
//! handlers that need a `UserId` for an FK write call the provider rather
//! than fabricate one.

crate::define_id!(UserId, schema);

impl UserId {
    pub fn anonymous() -> Self {
        Self("anonymous".to_owned())
    }
}
