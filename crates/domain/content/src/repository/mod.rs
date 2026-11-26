//! Content persistence: SQL-backed repositories for content, links, and search.
//!
//! [`ContentRepository`] owns content rows; [`ContentFileRepository`] the
//! links between content items and files; [`LinkRepository`] and
//! [`LinkAnalyticsRepository`] own campaign links and their click analytics;
//! [`SearchRepository`] backs full-text queries;
//! [`ContentAudienceReportRepository`] reads the content rows the audience
//! reports name. All access goes through
//! compile-time-verified query macros.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod audience_report;
pub mod content;
pub mod content_files;
pub mod link;
pub mod search;

pub use audience_report::ContentAudienceReportRepository;
pub use content::ContentRepository;
pub use content_files::ContentFileRepository;
pub use link::{LinkAnalyticsRepository, LinkRepository};
pub use search::SearchRepository;

use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct ContentRepositories {
    pub content: ContentRepository,
    pub search: SearchRepository,
    pub link: LinkRepository,
    pub link_analytics: LinkAnalyticsRepository,
}

impl ContentRepositories {
    pub fn new(db: &DbPool) -> Self {
        Self {
            content: ContentRepository::new(db),
            search: SearchRepository::new(db),
            link: LinkRepository::new(db),
            link_analytics: LinkAnalyticsRepository::new(db),
        }
    }
}
