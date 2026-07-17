//! JWT expiry inspection used to decide when a cloud session needs refreshing.
//!
//! Re-exports [`decode_expiry`], [`expires_within`], and [`is_expired`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod token;

pub use token::{decode_expiry, expires_within, is_expired};
