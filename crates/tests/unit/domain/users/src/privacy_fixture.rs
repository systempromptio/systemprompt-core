//! Privacy mutations use an owned database and the real reporting worker.

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
    pub async fn new() -> Option<Self> {
        systemprompt_test_fixtures::fixture_database_url().ok()?;
        systemprompt_test_fixtures::ensure_test_bootstrap();
        let database = systemprompt_test_fixtures::DisposableDb::installed("users_privacy_test")
            .await
            .expect("installed private user fixture");
        let pool = database.pool().await.expect("private pool");
        Some(Self { pool, database })
    }

    pub async fn drain(&self) {
        systemprompt_test_fixtures::drain_reporting(&self.pool)
            .await
            .expect("drain committed reporting evidence before privacy mutation");
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
