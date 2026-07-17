//! HATEOAS-style response envelopes for the public HTTP surface.
//!
//! Public re-exports:
//! - Primary envelopes: [`ApiResponse`], [`SingleResponse`],
//!   [`CollectionResponse`], [`ResponseMeta`], [`ResponseLinks`].
//! - Specialized envelopes: [`SuccessResponse`], [`CreatedResponse`],
//!   [`AcceptedResponse`], [`Link`], [`DiscoveryResponse`].
//! - Markdown envelopes: [`MarkdownFrontmatter`], [`MarkdownResponse`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod envelopes;
mod markdown;
mod specialized;

pub use envelopes::{ApiResponse, CollectionResponse, ResponseLinks, ResponseMeta, SingleResponse};
pub use markdown::{MarkdownFrontmatter, MarkdownResponse};
pub use specialized::{
    AcceptedResponse, CreatedResponse, DiscoveryResponse, Link, SuccessResponse,
};
