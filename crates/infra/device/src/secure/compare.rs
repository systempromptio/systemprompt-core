//! Comparison of secret bytes whose running time does not depend on where they
//! differ. Only the length is not treated as secret: inputs of different
//! lengths compare unequal at once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::hint::black_box;

#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= black_box(x ^ y);
    }
    black_box(diff) == 0
}
