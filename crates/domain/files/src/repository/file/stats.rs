use anyhow::{Context, Result};

use super::FileRepository;

#[derive(Debug, Clone, Copy)]
pub struct FileStats {
    pub total_files: i64,
    pub total_size_bytes: i64,
    pub ai_images_count: i64,
    pub image_count: i64,
    pub image_size_bytes: i64,
    pub document_count: i64,
    pub document_size_bytes: i64,
    pub audio_count: i64,
    pub audio_size_bytes: i64,
    pub video_count: i64,
    pub video_size_bytes: i64,
    pub other_count: i64,
    pub other_size_bytes: i64,
}

impl FileRepository {
    pub async fn get_stats(&self) -> Result<FileStats> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) as "total_files!",
                COALESCE(SUM(size_bytes), 0)::bigint as "total_size_bytes!",
                COUNT(*) FILTER (WHERE ai_content = true) as "ai_images_count!",
                COUNT(*) FILTER (WHERE mime_type LIKE 'image/%') as "image_count!",
                COALESCE(SUM(size_bytes) FILTER (WHERE mime_type LIKE 'image/%'), 0)::bigint as "image_size!",
                COUNT(*) FILTER (WHERE mime_type LIKE 'application/pdf' OR mime_type LIKE 'application/msword%' OR mime_type LIKE 'application/vnd.openxmlformats%' OR mime_type LIKE 'text/%') as "document_count!",
                COALESCE(SUM(size_bytes) FILTER (WHERE mime_type LIKE 'application/pdf' OR mime_type LIKE 'application/msword%' OR mime_type LIKE 'application/vnd.openxmlformats%' OR mime_type LIKE 'text/%'), 0)::bigint as "document_size!",
                COUNT(*) FILTER (WHERE mime_type LIKE 'audio/%') as "audio_count!",
                COALESCE(SUM(size_bytes) FILTER (WHERE mime_type LIKE 'audio/%'), 0)::bigint as "audio_size!",
                COUNT(*) FILTER (WHERE mime_type LIKE 'video/%') as "video_count!",
                COALESCE(SUM(size_bytes) FILTER (WHERE mime_type LIKE 'video/%'), 0)::bigint as "video_size!"
            FROM files
            WHERE deleted_at IS NULL
            "#
        )
        .fetch_one(self.pool.as_ref())
        .await
        .context("Failed to get file stats")?;

        let image_count = row.image_count;
        let document_count = row.document_count;
        let audio_count = row.audio_count;
        let video_count = row.video_count;
        let other_count =
            (row.total_files - image_count - document_count - audio_count - video_count).max(0);

        let image_size = row.image_size;
        let document_size = row.document_size;
        let audio_size = row.audio_size;
        let video_size = row.video_size;
        let other_size =
            (row.total_size_bytes - image_size - document_size - audio_size - video_size).max(0);

        Ok(FileStats {
            total_files: row.total_files,
            total_size_bytes: row.total_size_bytes,
            ai_images_count: row.ai_images_count,
            image_count,
            image_size_bytes: image_size,
            document_count,
            document_size_bytes: document_size,
            audio_count,
            audio_size_bytes: audio_size,
            video_count,
            video_size_bytes: video_size,
            other_count,
            other_size_bytes: other_size,
        })
    }
}
