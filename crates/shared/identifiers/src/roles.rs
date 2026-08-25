//! Role identifier.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// Why: role ids arrive from untrusted input at the access-control endpoints,
// which parse them with `try_new` and reject what does not validate. The
// `non_empty` arm keeps the infallible `new` for the call sites that hold a
// known-good literal, so this adds the fallible constructor without forcing a
// migration.
crate::define_id!(RoleId, non_empty);
