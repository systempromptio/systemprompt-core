//! Content persistence: SQL-backed repositories for content, links, and search.
//!
//! [`ContentRepository`] owns content rows; [`LinkRepository`] and
//! [`LinkAnalyticsRepository`] own campaign links and their click analytics;
//! [`SearchRepository`] backs full-text queries. All access goes through
//! compile-time-verified query macros.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod content;
pub mod link;
pub mod search;

pub use content::ContentRepository;
pub use link::{LinkAnalyticsRepository, LinkRepository};
pub use search::SearchRepository;
