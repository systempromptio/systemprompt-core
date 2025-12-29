use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

pub type PgDbPool = Arc<PgPool>;

#[async_trait]
pub trait Repository: Send + Sync {
    type Entity: Send + Sync;
    type Id: Send + Sync;
    type Error: Send + Sync + std::error::Error;

    fn pool(&self) -> &PgDbPool;

    async fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>, Self::Error>;

    async fn find_all(&self) -> Result<Vec<Self::Entity>, Self::Error>;

    async fn insert(&self, entity: &Self::Entity) -> Result<Self::Id, Self::Error>;

    async fn update(&self, entity: &Self::Entity) -> Result<(), Self::Error>;

    async fn delete(&self, id: &Self::Id) -> Result<(), Self::Error>;

    async fn exists(&self, id: &Self::Id) -> Result<bool, Self::Error> {
        Ok(self.find_by_id(id).await?.is_some())
    }

    async fn count(&self) -> Result<i64, Self::Error>;
}

#[async_trait]
pub trait SoftDeleteRepository: Repository {
    async fn soft_delete(&self, id: &Self::Id) -> Result<(), Self::Error>;

    async fn restore(&self, id: &Self::Id) -> Result<(), Self::Error>;

    async fn find_all_with_deleted(&self) -> Result<Vec<Self::Entity>, Self::Error>;
}

#[async_trait]
pub trait PaginatedRepository: Repository {
    async fn find_paginated(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Self::Entity>, Self::Error>;
}
