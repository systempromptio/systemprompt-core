//! An owner reassignment that keeps the target's counterpart row renders its
//! columns through fixed aliases, so a member table whose name is the alias
//! the statement once used (`kept`) is reassigned exactly like any other:
//! the source row with no counterpart moves, the one with a counterpart stays.

use std::sync::Arc;

use systemprompt_extension::{
    AdmittedOwnerReassignments, OwnerReassignment, ReassignAction, ReassignmentClaim,
    SchemaDefinition,
};
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};
use systemprompt_users::{UserRepository, UserService};

const KEPT_SQL: &str = "CREATE TABLE kept (owner_id TEXT NOT NULL, k TEXT NOT NULL, \
                        PRIMARY KEY (owner_id, k))";

async fn keys_of(pool: &sqlx::PgPool, owner: &UserId) -> Vec<String> {
    sqlx::query_scalar::<_, String>("SELECT k FROM kept WHERE owner_id = $1 ORDER BY k")
        .bind(owner.as_str())
        .fetch_all(pool)
        .await
        .expect("read kept")
}

#[tokio::test]
async fn a_member_table_named_like_a_statement_alias_is_reassigned_by_its_own_rows() {
    let database = DisposableDb::with_schema("owner_reassignment_alias").await;
    let db = database.test_pool().await;
    let raw = db.write_pool();
    sqlx::raw_sql(sqlx::AssertSqlSafe(KEPT_SQL))
        .execute(raw.as_ref())
        .await
        .expect("create kept");

    let source = UserId::new(format!("alias-source-{}", uuid::Uuid::new_v4()));
    let target = UserId::new(format!("alias-target-{}", uuid::Uuid::new_v4()));
    for owner in [&source, &target] {
        seed_user_row(&db, owner, &format!("{owner}@alias.invalid"))
            .await
            .expect("seed user");
    }
    for (owner, key) in [(&source, "shared"), (&source, "only-source"), (&target, "shared")] {
        sqlx::query("INSERT INTO kept (owner_id, k) VALUES ($1, $2)")
            .bind(owner.as_str())
            .bind(key)
            .execute(raw.as_ref())
            .await
            .expect("seed kept");
    }

    let owner = ExtensionId::new("alias_member");
    let schemas = [SchemaDefinition::new("kept", KEPT_SQL)];
    let reassignments = [OwnerReassignment {
        table: "kept",
        column: "owner_id",
        action: ReassignAction::Update {
            touch: None,
            keep_target_on: &["k"],
        },
    }];
    let admitted = AdmittedOwnerReassignments::admit(&[ReassignmentClaim {
        owner: &owner,
        schemas: &schemas,
        reassignments: &reassignments,
    }])
    .expect("the declaration is admitted");
    let service = UserService::new(Arc::new(UserRepository::new(&db)))
        .with_owner_reassignments(Arc::new(admitted));

    service
        .merge_users(&source, &target)
        .await
        .expect("the merge completes");

    assert_eq!(
        keys_of(raw.as_ref(), &target).await,
        ["only-source", "shared"],
        "the source row without a counterpart moves to the target"
    );
    assert_eq!(
        keys_of(raw.as_ref(), &source).await,
        ["shared"],
        "the source row whose counterpart the target holds stays in place"
    );

    drop(service);
    drop(raw);
    db.write_pool().close().await;
    drop(db);
    database.drop_now().await;
}
