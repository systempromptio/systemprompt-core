use anyhow::Result;
use systemprompt_core_database::{Database, DatabaseProvider};

pub struct TestCleanup {
    db: std::sync::Arc<Database>,
    fingerprints: Vec<String>,
    task_ids: Vec<String>,
    session_ids: Vec<String>,
}

impl TestCleanup {
    pub fn new(db: std::sync::Arc<Database>) -> Self {
        Self {
            db,
            fingerprints: Vec::new(),
            task_ids: Vec::new(),
            session_ids: Vec::new(),
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

        Ok(())
    }

    async fn cleanup_by_fingerprint(&self, fingerprint: &str) -> Result<()> {
        let rows = self
            .db
            .fetch_all(
                &"SELECT session_id FROM analytics_sessions WHERE fingerprint_hash = $1",
                &[&fingerprint],
            )
            .await?;

        for row in rows {
            if let Some(session_id) = row.get("session_id").and_then(|v| v.as_str()) {
                self.db
                    .execute(
                        &"DELETE FROM analytics_sessions WHERE session_id = $1",
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
                &"DELETE FROM analytics_sessions WHERE session_id = $1",
                &[&session_id],
            )
            .await?;
        Ok(())
    }
}

impl Drop for TestCleanup {
    fn drop(&mut self) {}
}
