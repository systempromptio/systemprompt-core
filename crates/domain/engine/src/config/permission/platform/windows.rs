//! Windows: patterns and subjects are compared in lower case.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Platform;

pub(super) struct Windows;

impl Platform for Windows {
    fn fold(chars: impl Iterator<Item = char>) -> Vec<char> {
        chars.flat_map(char::to_lowercase).collect()
    }
}
