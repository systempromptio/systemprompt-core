//! JWT expiry inspection used to decide when a cloud session needs refreshing.
//!
//! Re-exports [`decode_expiry`], [`expires_within`], and [`is_expired`].

mod token;

pub use token::{decode_expiry, expires_within, is_expired};
