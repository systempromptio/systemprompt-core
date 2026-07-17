//! User identifier.
//!
//! Every `UserId` must originate from a row in the `users` table. The
//! request middleware persists an anonymous user before constructing a
//! request context (see `SessionCreationService::ensure_anonymous_user`);
//! handlers that need a `UserId` for an FK write call the provider rather
//! than fabricate one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(UserId, schema);
