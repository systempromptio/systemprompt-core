//! HATEOAS-style response envelopes for the public HTTP surface.
//!
//! Public re-exports:
//! - Primary envelopes: [`ApiResponse`], [`SingleResponse`],
//!   [`CollectionResponse`], [`ResponseMeta`], [`ResponseLinks`].
//! - Specialized envelopes: [`SuccessResponse`], [`CreatedResponse`],
//!   [`AcceptedResponse`], [`Link`], [`DiscoveryResponse`].
//! - Markdown envelopes: [`MarkdownFrontmatter`], [`MarkdownResponse`].

mod envelopes;
mod markdown;
mod specialized;

pub use envelopes::{ApiResponse, CollectionResponse, ResponseLinks, ResponseMeta, SingleResponse};
pub use markdown::{MarkdownFrontmatter, MarkdownResponse};
pub use specialized::{
    AcceptedResponse, CreatedResponse, DiscoveryResponse, Link, SuccessResponse,
};
