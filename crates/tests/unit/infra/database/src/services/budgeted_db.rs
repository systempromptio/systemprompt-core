//! DB-backed tests for `BudgetedDb`: a job's budget is applied with
//! `SET LOCAL` to each transaction begun through it, cancels a statement
//! that outruns it, and never leaks into the pooled session afterwards.

use std::time::Duration;

use systemprompt_database::BudgetedDb;
use systemprompt_traits::DbBudget;

use super::db_helper::test_pool;

async fn setting(tx: &mut sqlx::PgConnection, name: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT current_setting($1)")
        .bind(name)
        .fetch_one(tx)
        .await
        .expect("read setting")
}

#[tokio::test]
async fn a_budget_is_set_local_to_each_transaction() {
    let db = test_pool().await;
    let budget = DbBudget::new(Duration::from_millis(1_500))
        .with_lock_timeout(Duration::from_millis(250))
        .with_work_mem_kb(8_192);
    let budgeted = BudgetedDb::new(db.clone(), Some(budget));

    let mut tx = budgeted.begin().await.expect("begin");
    assert_eq!(setting(&mut tx, "statement_timeout").await, "1500ms");
    assert_eq!(setting(&mut tx, "lock_timeout").await, "250ms");
    assert_eq!(setting(&mut tx, "work_mem").await, "8MB");
    tx.commit().await.expect("commit");

    let unbudgeted = BudgetedDb::new(db, None);
    let mut plain = unbudgeted.begin().await.expect("begin");
    assert_ne!(setting(&mut plain, "statement_timeout").await, "1500ms");
    plain.rollback().await.expect("rollback");
}

#[tokio::test]
async fn a_statement_past_the_budget_is_cancelled() {
    let db = test_pool().await;
    let budgeted = BudgetedDb::new(db, Some(DbBudget::new(Duration::from_millis(100))));
    let mut tx = budgeted.begin().await.expect("begin");
    let error = sqlx::query("SELECT pg_sleep(2)")
        .execute(&mut *tx)
        .await
        .expect_err("the statement outruns its budget");
    let code = error
        .as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.into_owned());
    assert_eq!(code.as_deref(), Some("57014"), "{error}");
}

#[test]
fn a_job_declares_no_budget_by_default() {
    struct Plain;
    #[async_trait::async_trait]
    impl systemprompt_traits::Job for Plain {
        fn name(&self) -> &'static str {
            "plain"
        }
        fn schedule(&self) -> &'static str {
            ""
        }
        async fn execute(
            &self,
            _ctx: &systemprompt_traits::JobContext,
        ) -> systemprompt_traits::ProviderResult<systemprompt_traits::JobResult> {
            Ok(systemprompt_traits::JobResult::success())
        }
    }
    assert_eq!(systemprompt_traits::Job::db_budget(&Plain), None);
}
