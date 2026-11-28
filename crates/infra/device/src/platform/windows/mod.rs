//! Windows FFI: DPAPI, owner-only security descriptors and the replacing
//! rename. The only module of the crate with `unsafe`; every block states why
//! it is sound.
//!
//! A private file carries a protected DACL granting full access to the current
//! user, SYSTEM and Administrators, nothing inherited. A private directory
//! carries the same entries marked `OICI`: Windows propagates a protected
//! directory DACL to every unprotected child, and a directory DACL without
//! inheritable entries would leave such children with an empty DACL that denies
//! even their owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod acl;
mod dpapi;
mod ffi;
mod filesystem;
mod identity;

pub(crate) use acl::{create_private, protect_directory, verify_private};
pub(crate) use dpapi::{dpapi_protect, dpapi_unprotect, is_foreign_ciphertext};
pub(crate) use filesystem::{is_reparse_point, is_transient_replace_error, move_replace};
pub(crate) use identity::current_sid;
