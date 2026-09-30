//! The connection-scope seam: scoped transactions apply registered providers'
//! GUCs inside the transaction only, isolated across concurrent scopes on a
//! shared pool, and the plain transaction APIs never touch them.

use std::sync::Arc;

use systemprompt_database::scope::{
    ConnectionScopeProvider, ScopeError, ScopeSetting, SharedScopeProvider,
};
use systemprompt_database::{
    RequestScope, register_scope_provider, with_scoped_transaction, with_transaction,
};
use systemprompt_test_fixtures::test_database_url;

struct OrgScopeProvider;

#[async_trait::async_trait]
impl ConnectionScopeProvider for OrgScopeProvider {
    async fn scope_settings(&self, scope: &RequestScope) -> Result<Vec<ScopeSetting>, ScopeError> {
        Ok(match scope.get("org_id") {
            Some(org) => vec![ScopeSetting::new("app.current_org", org)?],
            None => Vec::new(),
        })
    }
}

register_scope_provider!(|| Arc::new(OrgScopeProvider) as SharedScopeProvider);

async fn small_test_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&test_database_url())
        .await
        .expect("connect to the test database")
}

async fn current_org(executor: impl sqlx::PgExecutor<'_>) -> Option<String> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT current_setting('app.current_org', true)")
            .fetch_one(executor)
            .await
            .expect("current_setting");
    value.filter(|v| !v.is_empty())
}

fn org_scope(org: &str) -> RequestScope {
    let mut scope = RequestScope::new();
    scope.insert("org_id", org);
    scope
}

#[tokio::test]
async fn a_scoped_transaction_sees_its_guc_and_the_pool_is_clean_afterwards() {
    let pool = small_test_pool().await;

    let seen =
        with_scoped_transaction::<_, _, anyhow::Error>(&pool, &org_scope("org_alpha"), |tx| {
            Box::pin(async move { Ok(current_org(&mut **tx).await) })
        })
        .await
        .expect("scoped tx");
    assert_eq!(seen.as_deref(), Some("org_alpha"));

    // set_config(.., true) is transaction-local: a fresh checkout sees nothing.
    let mut conn = pool.acquire().await.expect("acquire");
    assert_eq!(current_org(&mut *conn).await, None);
}

#[tokio::test]
async fn concurrent_scopes_stay_isolated_on_a_shared_pool() {
    let pool = small_test_pool().await;

    // More tasks than pool connections (2), so connections are provably
    // reused across differently-scoped transactions.
    let mut handles = Vec::new();
    for i in 0..8 {
        let pool = pool.clone();
        let org = if i % 2 == 0 { "org_even" } else { "org_odd" };
        handles.push(tokio::spawn(async move {
            let seen =
                with_scoped_transaction::<_, _, anyhow::Error>(&pool, &org_scope(org), |tx| {
                    Box::pin(async move { Ok(current_org(&mut **tx).await) })
                })
                .await
                .expect("scoped tx");
            assert_eq!(seen.as_deref(), Some(org));
        }));
    }
    for handle in handles {
        handle.await.expect("task");
    }
}

#[tokio::test]
async fn a_rolled_back_scoped_transaction_leaves_no_guc_behind() {
    let pool = small_test_pool().await;

    let result =
        with_scoped_transaction::<_, (), anyhow::Error>(&pool, &org_scope("org_rollback"), |tx| {
            Box::pin(async move {
                assert_eq!(
                    current_org(&mut **tx).await.as_deref(),
                    Some("org_rollback")
                );
                Err(anyhow::anyhow!("force rollback"))
            })
        })
        .await;
    assert!(result.is_err());

    let mut conn = pool.acquire().await.expect("acquire");
    assert_eq!(current_org(&mut *conn).await, None);
}

#[tokio::test]
async fn plain_transactions_ignore_registered_providers() {
    let pool = small_test_pool().await;

    // A provider IS registered in this binary; the unscoped API must not
    // consult it.
    let seen = with_transaction::<_, _, anyhow::Error>(&pool, |tx| {
        Box::pin(async move { Ok(current_org(&mut **tx).await) })
    })
    .await
    .expect("plain tx");
    assert_eq!(seen, None);
}

#[tokio::test]
async fn an_empty_scope_applies_nothing() {
    let pool = small_test_pool().await;

    let seen = with_scoped_transaction::<_, _, anyhow::Error>(&pool, &RequestScope::new(), |tx| {
        Box::pin(async move { Ok(current_org(&mut **tx).await) })
    })
    .await
    .expect("scoped tx");
    assert_eq!(seen, None);
}
