use async_trait::async_trait;
use systemprompt_identifiers::{ContentId, LocaleCode, SourceId};
use systemprompt_traits::{ContentCatalogStats, RepositoryError};

#[derive(Debug, Clone, Copy)]
pub(crate) struct PublicPages(pub(crate) i64);

#[async_trait]
impl ContentCatalogStats for PublicPages {
    async fn count_public_pages(&self) -> Result<i64, RepositoryError> {
        Ok(self.0)
    }

    async fn has_content(
        &self,
        _source_id: &SourceId,
        _slug: &str,
        _locale: &LocaleCode,
    ) -> Result<bool, RepositoryError> {
        Ok(false)
    }

    async fn content_id_by_slug(
        &self,
        _slug: &str,
        _locale: &LocaleCode,
    ) -> Result<Option<ContentId>, RepositoryError> {
        Ok(None)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnreadableCatalog;

#[async_trait]
impl ContentCatalogStats for UnreadableCatalog {
    async fn count_public_pages(&self) -> Result<i64, RepositoryError> {
        Err(RepositoryError::invalid_argument("catalog", "unreadable"))
    }

    async fn has_content(
        &self,
        _source_id: &SourceId,
        _slug: &str,
        _locale: &LocaleCode,
    ) -> Result<bool, RepositoryError> {
        Err(RepositoryError::invalid_argument("catalog", "unreadable"))
    }

    async fn content_id_by_slug(
        &self,
        _slug: &str,
        _locale: &LocaleCode,
    ) -> Result<Option<ContentId>, RepositoryError> {
        Err(RepositoryError::invalid_argument("catalog", "unreadable"))
    }
}
