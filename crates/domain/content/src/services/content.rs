use crate::error::ContentError;
use crate::models::Content;
use crate::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;

#[derive(Debug)]
pub struct ContentService {
    repo: ContentRepository,
}

impl ContentService {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        Ok(Self {
            repo: ContentRepository::new(db)?,
        })
    }

    pub async fn list_by_source(&self, source_id: &SourceId) -> Result<Vec<Content>, ContentError> {
        self.repo
            .list_by_source(source_id)
            .await
            .map_err(ContentError::from)
    }

    pub async fn get_by_source_and_slug(
        &self,
        source_id: &SourceId,
        slug: &str,
    ) -> Result<Option<Content>, ContentError> {
        self.repo
            .get_by_source_and_slug(source_id, slug)
            .await
            .map_err(ContentError::from)
    }
}
