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

use crate::error::ContentError;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct ContentRepositories {
    pub content: ContentRepository,
    pub search: SearchRepository,
    pub link: LinkRepository,
    pub link_analytics: LinkAnalyticsRepository,
}

impl ContentRepositories {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        Ok(Self {
            content: ContentRepository::new(db)?,
            search: SearchRepository::new(db)?,
            link: LinkRepository::new(db)?,
            link_analytics: LinkAnalyticsRepository::new(db)?,
        })
    }
}
