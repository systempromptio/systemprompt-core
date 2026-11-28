//! Unix: patterns and subjects are compared as written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Platform;

pub(super) struct Unix;

impl Platform for Unix {
    fn fold(chars: impl Iterator<Item = char>) -> Vec<char> {
        chars.collect()
    }
}
