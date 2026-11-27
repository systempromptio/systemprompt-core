//! macOS Keychain through the safe `security-framework` bindings: generic
//! password items in the user's default (login) keychain, one of which holds
//! the sealing key. No `unsafe` here.
//!
//! An item's access list and partition trust the binary that created it. A
//! build with another code signature (every ad-hoc signed rebuild) is asked
//! about by the system on its first read of an existing item, so that read
//! may wait on the user once per build.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod passwords;
mod sealer;
mod sealing_key;
mod status;

pub(crate) use passwords::{default_keychain, delete_password, load_password, store_password};
pub use sealer::KeychainSealer;
