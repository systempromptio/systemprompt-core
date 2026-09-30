//! Privacy mutations use an owned, disposable database.

pub(crate) struct PrivacyFixture {
    pub pool: systemprompt_database::DbPool,
    database: systemprompt_test_fixtures::DisposableDb,
}

impl std::fmt::Debug for PrivacyFixture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivacyFixture").finish_non_exhaustive()
    }
}

impl PrivacyFixture {
    pub async fn new() -> Self {
        systemprompt_test_fixtures::ensure_test_bootstrap();
        let database =
            systemprompt_test_fixtures::DisposableDb::with_schema("users_privacy_test").await;
        let pool = database.test_pool().await;
        Self { pool, database }
    }

    pub async fn finish(self) {
        self.pool
            .write_pool_arc()
            .expect("write pool")
            .close()
            .await;
        self.database.drop_now().await;
    }
}
