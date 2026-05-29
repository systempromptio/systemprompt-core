//! Parameter builders for content and link mutations.
//!
//! Groups the create/update parameter types consumed by the repository layer:
//! content via [`CreateContentParams`] / [`UpdateContentParams`], and links via
//! [`CreateLinkParams`], [`RecordClickParams`], and [`TrackClickParams`].

pub mod content;
pub mod link;

pub use content::{CategoryIdUpdate, CreateContentParams, UpdateContentParams};
pub use link::{CreateLinkParams, RecordClickParams, TrackClickParams};
