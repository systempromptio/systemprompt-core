//! CSRF token minting and constant-time comparison for the GUI loopback server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use rand::Rng as _;

pub(crate) fn mint_csrf_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    crate::hash::hex_encode(&bytes)
}

pub(crate) use crate::proxy::secret::constant_time_eq;
