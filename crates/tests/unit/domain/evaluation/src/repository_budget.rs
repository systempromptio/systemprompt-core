//! DB-backed tests for the experiment budget repository: reservation
//! admission, idempotent settlement and the freeze that follows an overspend.
//! Every test owns a fresh UUID owner and its own budget account, so no
//! assertion depends on shared-table state.

use sqlx::PgPool;
use systemprompt_evaluation::EvaluationError;
use systemprompt_evaluation::repository::experiments::{BudgetRepository, ReservationAdmission};
use systemprompt_identifiers::{AiRequestId, EvalBudgetId, EvalReservationId, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn budget_pool() -> Option<PgPool> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let write = pool.write_pool_arc().expect("write pool");
    Some(write.as_ref().clone())
}

async fn new_owner(pool: &PgPool) -> UserId {
    let owner = UserId::new(format!("eval-budget-{}", Uuid::new_v4()));
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(owner.as_str())
        .bind(format!("{}@eval.invalid", owner.as_str()))
        .execute(pool)
        .await
        .expect("seed owner");
    owner
}

fn new_request() -> AiRequestId {
    AiRequestId::new(format!("eval-budget-req-{}", Uuid::new_v4()))
}

async fn create(
    budgets: &BudgetRepository,
    owner: &UserId,
    cap: i64,
) -> systemprompt_evaluation::Result<EvalBudgetId> {
    budgets
        .create_shared(owner, &format!("account-{}", Uuid::new_v4()), cap)
        .await
}

struct Account {
    reserved: i64,
    settled: i64,
    frozen: bool,
}

async fn account(pool: &PgPool, id: &EvalBudgetId) -> Account {
    let row: (i64, i64, bool) =
        sqlx::query_as("SELECT reserved, settled, frozen FROM eval_budget_accounts WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(pool)
            .await
            .expect("account");
    Account {
        reserved: row.0,
        settled: row.1,
        frozen: row.2,
    }
}

#[tokio::test]
async fn create_rejects_non_positive_caps_and_persists_positive_ones() {
    let Some(pool) = budget_pool().await else {
        return;
    };
    let budgets = crate::seams::budgets(&pool);
    let owner = new_owner(&pool).await;

    for cap in [0, -1] {
        assert!(
            matches!(
                create(&budgets, &owner, cap).await,
                Err(EvaluationError::InvalidSpec(_))
            ),
            "cap {cap} must be rejected"
        );
    }

    let id = create(&budgets, &owner, 5_000).await.expect("create");
    let state = account(&pool, &id).await;
    assert_eq!(state.reserved, 0);
    assert_eq!(state.settled, 0);
    assert!(!state.frozen);
}

#[tokio::test]
async fn reservation_rejects_malformed_bounds_and_unknown_accounts() {
    let Some(pool) = budget_pool().await else {
        return;
    };
    let budgets = crate::seams::budgets(&pool);
    let owner = new_owner(&pool).await;
    let account_id = create(&budgets, &owner, 5_000).await.expect("create");

    for (operation, amount) in [("op", 0), ("op", -5), ("", 10), ("   ", 10)] {
        assert!(
            matches!(
                budgets
                    .reserve(&owner, &account_id, operation, amount)
                    .await,
                Err(EvaluationError::InvalidSpec(_))
            ),
            "operation {operation:?} amount {amount} must be rejected"
        );
    }
    let long_key = "k".repeat(256);
    assert!(matches!(
        budgets.reserve(&owner, &account_id, &long_key, 10).await,
        Err(EvaluationError::InvalidSpec(_))
    ));

    assert!(matches!(
        budgets
            .reserve(&owner, &EvalBudgetId::generate(), "op", 10)
            .await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
    assert!(
        matches!(
            budgets
                .reserve(&new_owner(&pool).await, &account_id, "op", 10)
                .await,
            Err(EvaluationError::ResourceNotFound(_))
        ),
        "another owner must not see the account"
    );
}

#[tokio::test]
async fn reservation_is_idempotent_per_operation_key_and_conflicts_on_a_new_bound() {
    let Some(pool) = budget_pool().await else {
        return;
    };
    let budgets = crate::seams::budgets(&pool);
    let owner = new_owner(&pool).await;
    let account_id = create(&budgets, &owner, 5_000).await.expect("create");

    let first = budgets
        .reserve(&owner, &account_id, "dispatch-1", 400)
        .await
        .expect("reserve");
    let ReservationAdmission::Admitted(reservation) = first else {
        panic!("first reservation must be admitted: {first:?}");
    };
    assert_eq!(account(&pool, &account_id).await.reserved, 400);

    let repeat = budgets
        .reserve(&owner, &account_id, "dispatch-1", 400)
        .await
        .expect("repeat");
    assert_eq!(
        repeat,
        ReservationAdmission::AlreadyReserved(reservation),
        "a repeated key must never admit a second dispatch"
    );
    assert_eq!(
        account(&pool, &account_id).await.reserved,
        400,
        "reconciliation must not double-hold"
    );

    assert!(matches!(
        budgets
            .reserve(&owner, &account_id, "dispatch-1", 401)
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));
}

#[tokio::test]
async fn reservation_stops_at_the_cap_and_at_a_frozen_account() {
    let Some(pool) = budget_pool().await else {
        return;
    };
    let budgets = crate::seams::budgets(&pool);
    let owner = new_owner(&pool).await;
    let account_id = create(&budgets, &owner, 1_000).await.expect("create");

    budgets
        .reserve(&owner, &account_id, "hold", 900)
        .await
        .expect("reserve");
    match budgets.reserve(&owner, &account_id, "over", 200).await {
        Err(EvaluationError::BudgetExhausted {
            required,
            available,
        }) => {
            assert_eq!(required, 200);
            assert_eq!(available, 100);
        },
        other => panic!("cap breach must be refused: {other:?}"),
    }
    budgets
        .reserve(&owner, &account_id, "exact", 100)
        .await
        .expect("a reservation reaching the cap exactly is admitted");

    sqlx::query("UPDATE eval_budget_accounts SET frozen = TRUE, reserved = 0 WHERE id = $1")
        .bind(account_id.as_str())
        .execute(&pool)
        .await
        .expect("freeze");
    assert!(matches!(
        budgets
            .reserve(&owner, &account_id, "after-freeze", 1)
            .await,
        Err(EvaluationError::BudgetExhausted { .. })
    ));
}

#[tokio::test]
async fn concurrent_reservations_cannot_overdraw_the_shared_account() {
    let Some(pool) = budget_pool().await else {
        return;
    };
    let budgets = crate::seams::budgets(&pool);
    let owner = new_owner(&pool).await;
    let account_id = create(&budgets, &owner, 100).await.expect("create");
    let first = budgets.clone();
    let second = budgets.clone();
    let first_owner = owner.clone();
    let second_owner = owner.clone();
    let first_account = account_id.clone();
    let second_account = account_id.clone();
    let (left, right) = tokio::join!(
        first.reserve(&first_owner, &first_account, "contender-left", 80),
        second.reserve(&second_owner, &second_account, "contender-right", 80),
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert_eq!(account(&pool, &account_id).await.reserved, 80);
}

#[tokio::test]
async fn settlement_moves_reserved_to_settled_exactly_once() {
    let Some(pool) = budget_pool().await else {
        return;
    };
    let budgets = crate::seams::budgets(&pool);
    let owner = new_owner(&pool).await;
    let account_id = create(&budgets, &owner, 5_000).await.expect("create");
    let ReservationAdmission::Admitted(reservation) = budgets
        .reserve(&owner, &account_id, "dispatch", 400)
        .await
        .expect("reserve")
    else {
        panic!("expected admission");
    };
    let request = new_request();

    budgets
        .settle(&owner, &reservation, &request, 250)
        .await
        .expect("settle");
    let state = account(&pool, &account_id).await;
    assert_eq!(state.reserved, 0);
    assert_eq!(state.settled, 250);
    assert!(!state.frozen, "spend within the bound must not freeze");

    budgets
        .settle(&owner, &reservation, &request, 250)
        .await
        .expect("replayed settlement is a no-op");
    let state = account(&pool, &account_id).await;
    assert_eq!(state.settled, 250, "settlement must not apply twice");
}

#[tokio::test]
async fn settlement_rejects_negative_unknown_and_contradictory_spend() {
    let Some(pool) = budget_pool().await else {
        return;
    };
    let budgets = crate::seams::budgets(&pool);
    let owner = new_owner(&pool).await;
    let account_id = create(&budgets, &owner, 5_000).await.expect("create");
    let ReservationAdmission::Admitted(reservation) = budgets
        .reserve(&owner, &account_id, "dispatch", 400)
        .await
        .expect("reserve")
    else {
        panic!("expected admission");
    };
    let request = new_request();

    assert!(matches!(
        budgets.settle(&owner, &reservation, &request, -1).await,
        Err(EvaluationError::InvalidSpec(_))
    ));
    assert!(matches!(
        budgets
            .settle(&owner, &EvalReservationId::generate(), &request, 10)
            .await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
    assert!(
        matches!(
            budgets
                .settle(&new_owner(&pool).await, &reservation, &request, 10)
                .await,
            Err(EvaluationError::ResourceNotFound(_))
        ),
        "another owner must not settle this reservation"
    );

    budgets
        .settle(&owner, &reservation, &request, 100)
        .await
        .expect("settle");
    assert!(
        matches!(
            budgets.settle(&owner, &reservation, &request, 120).await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "a different amount must not overwrite recorded spend"
    );
    assert!(
        matches!(
            budgets
                .settle(&owner, &reservation, &new_request(), 100)
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "a different request must not claim the same settlement"
    );
}

#[tokio::test]
async fn overspending_a_reservation_freezes_the_account() {
    let Some(pool) = budget_pool().await else {
        return;
    };
    let budgets = crate::seams::budgets(&pool);
    let owner = new_owner(&pool).await;
    let account_id = create(&budgets, &owner, 5_000).await.expect("create");
    let ReservationAdmission::Admitted(reservation) = budgets
        .reserve(&owner, &account_id, "dispatch", 100)
        .await
        .expect("reserve")
    else {
        panic!("expected admission");
    };

    budgets
        .settle(&owner, &reservation, &new_request(), 300)
        .await
        .expect("settle");
    let state = account(&pool, &account_id).await;
    assert_eq!(state.settled, 300);
    assert!(
        state.frozen,
        "spend beyond the bound must freeze the account"
    );
    assert!(
        matches!(
            budgets.reserve(&owner, &account_id, "next", 1).await,
            Err(EvaluationError::BudgetExhausted { .. })
        ),
        "a frozen account admits nothing further"
    );
}
