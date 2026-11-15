//! The `Extension` trait: a member's metadata and its one declaration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::contribute::{ContributionError, Contributions};
use crate::metadata::ExtensionMetadata;

/// A composition member: its metadata and everything it contributes.
///
/// `contribute` runs once per composition, before the profile is loaded, and
/// only stores into the member's sink: nothing it declares runs until the
/// composition has admitted the member. The member's identity is its Rust
/// type and its standing in the composition, never a string it supplies; the
/// metadata id is validated into an `ExtensionId` once, at composition.
pub trait Extension: Send + Sync + 'static {
    fn metadata(&self) -> ExtensionMetadata;
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError>;
}
