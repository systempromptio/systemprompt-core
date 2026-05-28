//! Verifies migration 010 has installed the
//! `oauth_clients_owner_user_id_fkey` constraint on the test pool. Catches
//! the regression where migration 004's `ADD COLUMN IF NOT EXISTS` silently
//! skipped the FK on legacy databases.

use anyhow::Result;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[tokio::test]
async fn oauth_clients_owner_user_id_fkey_is_installed() -> Result<()> {
    let url = fixture_database_url()?;
    let pool = fixture_db_pool(&url).await?;

    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
              SELECT 1 FROM pg_constraint
               WHERE conname = 'oauth_clients_owner_user_id_fkey'
                 AND conrelid = 'oauth_clients'::regclass
           )"#,
    )
    .fetch_one(pool.as_ref())
    .await?;

    assert!(
        exists,
        "expected oauth_clients_owner_user_id_fkey FK to be installed by migration 010"
    );

    Ok(())
}
