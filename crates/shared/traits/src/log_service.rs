use async_trait::async_trait;

#[async_trait]
pub trait LogService: Send + Sync {
    type Entry: Send + Sync;
    type Filter: Send + Sync;
    type Error: std::error::Error + Send + Sync;

    async fn log(&self, entry: Self::Entry) -> Result<(), Self::Error>;

    async fn query(&self, filter: &Self::Filter) -> Result<(Vec<Self::Entry>, i64), Self::Error>;

    async fn get_recent(&self, limit: i64) -> Result<Vec<Self::Entry>, Self::Error>;

    async fn get_by_id(&self, id: &str) -> Result<Option<Self::Entry>, Self::Error>;

    async fn delete(&self, id: &str) -> Result<bool, Self::Error>;
}
