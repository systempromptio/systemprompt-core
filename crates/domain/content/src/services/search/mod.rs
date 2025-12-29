use crate::error::ContentError;
use crate::models::{SearchRequest, SearchResponse, SearchResult};
use crate::repository::{ContentRepository, SearchRepository};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::CategoryId;

const DEFAULT_SEARCH_LIMIT: i64 = 10;

#[derive(Debug)]
pub struct SearchService {
    search_repo: SearchRepository,
    content_repo: ContentRepository,
}

impl SearchService {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        Ok(Self {
            search_repo: SearchRepository::new(db)?,
            content_repo: ContentRepository::new(db)?,
        })
    }

    pub async fn search(&self, request: &SearchRequest) -> Result<SearchResponse, ContentError> {
        let limit = request.limit.unwrap_or(DEFAULT_SEARCH_LIMIT);

        let results = if let Some(filters) = &request.filters {
            if let Some(category_id) = &filters.category_id {
                self.search_repo
                    .search_by_category(category_id, limit)
                    .await?
            } else {
                vec![]
            }
        } else {
            let content_list = self.content_repo.list_all(limit, 0).await?;
            content_list
                .into_iter()
                .map(Self::content_to_search_result)
                .collect()
        };

        Ok(SearchResponse {
            total: results.len(),
            results,
        })
    }

    pub async fn search_by_category(
        &self,
        category_id: &CategoryId,
        limit: i64,
    ) -> Result<Vec<SearchResult>, ContentError> {
        Ok(self
            .search_repo
            .search_by_category(category_id, limit)
            .await?)
    }

    fn content_to_search_result(content: crate::models::Content) -> SearchResult {
        SearchResult {
            id: content.id,
            title: content.title,
            slug: content.slug,
            description: content.description,
            image: content.image,
            view_count: 0,
            source_id: content.source_id,
            category_id: content.category_id,
        }
    }
}
