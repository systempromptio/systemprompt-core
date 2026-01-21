use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_traits::content::{ContentFilter, ContentItem, ContentProvider, ContentSummary};

use crate::error::ContentError;
use crate::repository::{ContentRepository, SearchRepository};

#[derive(Debug)]
pub struct DefaultContentProvider {
    repo: ContentRepository,
    search_repo: SearchRepository,
}

impl DefaultContentProvider {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        Ok(Self {
            repo: ContentRepository::new(db)?,
            search_repo: SearchRepository::new(db)?,
        })
    }
}

#[async_trait]
impl ContentProvider for DefaultContentProvider {
    type Error = ContentError;

    async fn get_content(&self, id: &str) -> Result<Option<ContentItem>, Self::Error> {
        let content_id = systemprompt_identifiers::ContentId::new(id);
        let content = self.repo.get_by_id(&content_id).await?;

        Ok(content.map(|c| ContentItem {
            id: c.id.to_string(),
            slug: c.slug,
            title: c.title,
            description: c.description,
            body: c.body,
            author: c.author,
            published_at: c.published_at,
            keywords: c.keywords,
            kind: c.kind,
            image: c.image,
            source_id: c.source_id.to_string(),
            category_id: c.category_id.map(|id| id.to_string()),
        }))
    }

    async fn get_content_by_slug(&self, slug: &str) -> Result<Option<ContentItem>, Self::Error> {
        let content = self.repo.get_by_slug(slug).await?;

        Ok(content.map(|c| ContentItem {
            id: c.id.to_string(),
            slug: c.slug,
            title: c.title,
            description: c.description,
            body: c.body,
            author: c.author,
            published_at: c.published_at,
            keywords: c.keywords,
            kind: c.kind,
            image: c.image,
            source_id: c.source_id.to_string(),
            category_id: c.category_id.map(|id| id.to_string()),
        }))
    }

    async fn get_content_by_source_and_slug(
        &self,
        source_id: &str,
        slug: &str,
    ) -> Result<Option<ContentItem>, Self::Error> {
        let source = systemprompt_identifiers::SourceId::new(source_id);
        let content = self.repo.get_by_source_and_slug(&source, slug).await?;

        Ok(content.map(|c| ContentItem {
            id: c.id.to_string(),
            slug: c.slug,
            title: c.title,
            description: c.description,
            body: c.body,
            author: c.author,
            published_at: c.published_at,
            keywords: c.keywords,
            kind: c.kind,
            image: c.image,
            source_id: c.source_id.to_string(),
            category_id: c.category_id.map(|id| id.to_string()),
        }))
    }

    async fn list_content(
        &self,
        filter: ContentFilter,
    ) -> Result<Vec<ContentSummary>, Self::Error> {
        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let contents = if let Some(source_id) = filter.source_id {
            let source = systemprompt_identifiers::SourceId::new(&source_id);
            self.repo.list_by_source(&source).await?
        } else {
            self.repo.list(limit, offset).await?
        };

        Ok(contents
            .into_iter()
            .map(|c| ContentSummary {
                id: c.id.to_string(),
                slug: c.slug,
                title: c.title,
                description: c.description,
                published_at: c.published_at,
                kind: c.kind,
                source_id: c.source_id.to_string(),
            })
            .collect())
    }

    async fn search(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ContentSummary>, Self::Error> {
        let limit = limit.unwrap_or(50);
        let results = self.search_repo.search_by_keyword(query, limit).await?;

        Ok(results
            .into_iter()
            .map(|r| ContentSummary {
                id: r.id.to_string(),
                slug: r.slug,
                title: r.title,
                description: r.description,
                published_at: chrono::Utc::now(),
                kind: String::new(),
                source_id: r.source_id.to_string(),
            })
            .collect())
    }
}
