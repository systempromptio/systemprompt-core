use anyhow::Result;
use systemprompt_database::{Database, DatabaseProvider};

pub const TEST_SOURCE_PREFIX: &str = "test_";

pub struct TestCleanup {
    db: std::sync::Arc<Database>,
    fingerprints: Vec<String>,
    task_ids: Vec<String>,
    session_ids: Vec<String>,
    content_slugs: Vec<(String, String)>,
    source_ids: Vec<String>,
}

impl TestCleanup {
    pub fn new(db: std::sync::Arc<Database>) -> Self {
        Self {
            db,
            fingerprints: Vec::new(),
            task_ids: Vec::new(),
            session_ids: Vec::new(),
            content_slugs: Vec::new(),
            source_ids: Vec::new(),
        }
    }

    pub fn track_fingerprint(&mut self, fingerprint: String) {
        self.fingerprints.push(fingerprint);
    }

    pub fn track_task(&mut self, task_id: String) {
        self.task_ids.push(task_id);
    }

    pub fn track_task_id(&mut self, task_id: String) {
        self.task_ids.push(task_id);
    }

    pub fn track_session(&mut self, session_id: String) {
        self.session_ids.push(session_id);
    }

    pub fn track_content(&mut self, source_id: String, slug: String) {
        self.content_slugs.push((source_id, slug));
    }

    pub fn track_source(&mut self, source_id: String) {
        self.source_ids.push(source_id);
    }

    pub async fn cleanup_all(&self) -> Result<()> {
        for fingerprint in &self.fingerprints {
            self.cleanup_by_fingerprint(fingerprint).await.ok();
        }

        for task_id in &self.task_ids {
            self.cleanup_task(task_id).await.ok();
        }

        for session_id in &self.session_ids {
            self.cleanup_session(session_id).await.ok();
        }

        for (source_id, slug) in &self.content_slugs {
            self.cleanup_content(source_id, slug).await.ok();
        }

        for source_id in &self.source_ids {
            self.cleanup_source_content(source_id).await.ok();
        }

        self.cleanup_test_sources().await.ok();

        Ok(())
    }

    async fn cleanup_content(&self, source_id: &str, slug: &str) -> Result<()> {
        self.db
            .execute(
                &"DELETE FROM markdown_content WHERE source_id = $1 AND slug = $2",
                &[&source_id, &slug],
            )
            .await?;
        Ok(())
    }

    async fn cleanup_source_content(&self, source_id: &str) -> Result<()> {
        self.db
            .execute(
                &"DELETE FROM markdown_content WHERE source_id = $1",
                &[&source_id],
            )
            .await?;
        Ok(())
    }

    async fn cleanup_test_sources(&self) -> Result<()> {
        let pattern = format!("{}%", TEST_SOURCE_PREFIX);
        self.db
            .execute(
                &"DELETE FROM markdown_content WHERE source_id LIKE $1",
                &[&pattern],
            )
            .await?;
        Ok(())
    }

    async fn cleanup_by_fingerprint(&self, fingerprint: &str) -> Result<()> {
        let rows = self
            .db
            .fetch_all(
                &"SELECT session_id FROM user_sessions WHERE fingerprint_hash = $1",
                &[&fingerprint],
            )
            .await?;

        for row in rows {
            if let Some(session_id) = row.get("session_id").and_then(|v| v.as_str()) {
                self.db
                    .execute(
                        &"DELETE FROM user_sessions WHERE session_id = $1",
                        &[&session_id],
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn cleanup_task(&self, _task_id: &str) -> Result<()> {
        Ok(())
    }

    async fn cleanup_session(&self, session_id: &str) -> Result<()> {
        self.db
            .execute(
                &"DELETE FROM user_sessions WHERE session_id = $1",
                &[&session_id],
            )
            .await?;
        Ok(())
    }
}

impl Drop for TestCleanup {
    fn drop(&mut self) {}
}
