//! Tests for the `FingerprintProvider for FingerprintRepository` bridge in
//! `services/providers.rs`. Happy paths run against the migrated DB and
//! assert the translated return values; every error arm is driven through a
//! closed pool to exercise the `Internal(e.to_string())` mapping.

use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};
use systemprompt_traits::{AnalyticsProviderError, FingerprintProvider};
use uuid::Uuid;

mod fingerprint_provider {
    use super::*;

    #[tokio::test]
    async fn upsert_then_count_and_reuse() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let repo = systemprompt_test_fixtures::fixture_fingerprint_repository(&pool).expect("repo");

        let fp = format!("fp-{}", Uuid::new_v4());
        FingerprintProvider::upsert_fingerprint(
            &repo,
            &fp,
            Some("1.2.3.4"),
            Some("Mozilla/5.0"),
            None,
        )
        .await
        .expect("upsert");

        let count = FingerprintProvider::count_active_sessions(&repo, &fp)
            .await
            .expect("count");
        assert!(count >= 0);
        // No active session references this fingerprint yet.
        assert!(
            FingerprintProvider::find_reusable_session(&repo, &fp)
                .await
                .expect("reuse")
                .is_none()
        );
    }

    #[tokio::test]
    async fn error_arms_map_to_internal() {
        let pool = closed_db_pool().await;
        let repo = systemprompt_test_fixtures::fixture_fingerprint_repository(&pool).expect("repo");

        assert!(matches!(
            FingerprintProvider::count_active_sessions(&repo, "fp").await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            FingerprintProvider::find_reusable_session(&repo, "fp").await,
            Err(AnalyticsProviderError::Internal(_))
        ));
        assert!(matches!(
            FingerprintProvider::upsert_fingerprint(&repo, "fp", None, None, None).await,
            Err(AnalyticsProviderError::Internal(_))
        ));
    }
}
