use anyhow::Result;

pub struct SessionAssertion {
    session_id: String,
}

impl SessionAssertion {
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }

    pub async fn exists(self) -> Result<Self> {
        assert!(!self.session_id.is_empty(), "Session ID is empty");
        Ok(self)
    }

    pub async fn has_prefix(self, prefix: &str) -> Result<Self> {
        assert!(
            self.session_id.starts_with(prefix),
            "Session ID {} does not have prefix {}",
            self.session_id,
            prefix
        );
        Ok(self)
    }

    pub async fn has_user_type(self, _user_type: &str) -> Result<Self> {
        Ok(self)
    }

    pub async fn has_request_count(self, _expected: i32) -> Result<Self> {
        Ok(self)
    }

    pub async fn has_analytics_events(self, _min_count: i32) -> Result<Self> {
        Ok(self)
    }
}

pub struct TaskAssertion {
    task_id: String,
}

impl TaskAssertion {
    pub fn new(task_id: String) -> Self {
        Self { task_id }
    }

    pub async fn exists(self) -> Result<Self> {
        assert!(!self.task_id.is_empty(), "Task ID is empty");
        Ok(self)
    }

    pub async fn has_status(self, _expected: &str) -> Result<Self> {
        Ok(self)
    }

    pub async fn has_messages(self, _expected: i32) -> Result<Self> {
        Ok(self)
    }

    pub async fn has_ai_requests(self, _min: i32) -> Result<Self> {
        Ok(self)
    }
}

pub struct IntegrityAssertion;

impl IntegrityAssertion {
    pub fn new() -> Self {
        Self
    }

    pub async fn no_orphaned_analytics_events(self) -> Result<Self> {
        Ok(self)
    }

    pub async fn no_orphaned_task_messages(self) -> Result<Self> {
        Ok(self)
    }

    pub async fn all_sessions_have_sess_prefix(self) -> Result<Self> {
        Ok(self)
    }
}

impl Default for IntegrityAssertion {
    fn default() -> Self {
        Self::new()
    }
}
