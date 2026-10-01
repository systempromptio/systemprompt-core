//! User identifier — a UUID.
//!
//! Every `UserId` names a row in the `users` table, whose ids are minted as
//! UUIDs. `try_new` validates that shape and is the constructor for values
//! arriving from outside (a JWT `sub`, a header, a path segment); `new` is
//! for values already known to be valid, such as a decoded `users.id`. The
//! request middleware persists an anonymous user before constructing a
//! request context, so handlers that need a `UserId` for an FK write call
//! the provider rather than fabricate one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(UserId, uuid);
