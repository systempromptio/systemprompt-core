use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSummary {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub published_at: DateTime<Utc>,
    pub kind: String,
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub author: String,
    pub published_at: DateTime<Utc>,
    pub keywords: String,
    pub kind: String,
    pub image: Option<String>,
    pub source_id: String,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFilter {
    pub source_id: Option<String>,
    pub category_id: Option<String>,
    pub kind: Option<String>,
    pub query: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[async_trait]
pub trait ContentProvider: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn get_content(&self, id: &str) -> Result<Option<ContentItem>, Self::Error>;

    async fn get_content_by_slug(&self, slug: &str) -> Result<Option<ContentItem>, Self::Error>;

    async fn get_content_by_source_and_slug(
        &self,
        source_id: &str,
        slug: &str,
    ) -> Result<Option<ContentItem>, Self::Error>;

    async fn list_content(&self, filter: ContentFilter)
        -> Result<Vec<ContentSummary>, Self::Error>;

    async fn search(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ContentSummary>, Self::Error>;
}
