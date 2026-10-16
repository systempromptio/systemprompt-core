use futures::TryStreamExt;
use systemprompt_traits::{ByteRange, FileStorage, FileStorageResult, StoredFileId};

pub(crate) trait ReadAll {
    async fn read_all(&self, id: &StoredFileId) -> FileStorageResult<Vec<u8>>;
    async fn read_range(&self, id: &StoredFileId, range: ByteRange) -> FileStorageResult<Vec<u8>>;
}

impl<T: FileStorage + ?Sized> ReadAll for T {
    async fn read_all(&self, id: &StoredFileId) -> FileStorageResult<Vec<u8>> {
        let read = self.open(id, None).await?;
        let chunks: Vec<bytes::Bytes> = read.body.try_collect().await?;
        Ok(chunks.concat())
    }

    async fn read_range(&self, id: &StoredFileId, range: ByteRange) -> FileStorageResult<Vec<u8>> {
        let read = self.open(id, Some(range)).await?;
        let chunks: Vec<bytes::Bytes> = read.body.try_collect().await?;
        Ok(chunks.concat())
    }
}
