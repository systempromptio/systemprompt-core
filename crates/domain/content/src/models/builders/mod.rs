//! Parameter builders for content and link mutations.
//!
//! Groups the create/update parameter types consumed by the repository layer:
//! content via [`CreateContentParams`] / [`UpdateContentParams`], and links via
//! [`CreateLinkParams`], [`RecordClickParams`], and [`TrackClickParams`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod content;
pub mod link;

pub use content::{CategoryIdUpdate, CreateContentParams, UpdateContentParams};
pub use link::{CreateLinkParams, RecordClickParams, TrackClickParams};
