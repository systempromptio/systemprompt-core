//! DB-backed tests for the recorded half of the user purge: a member absent
//! from the composition is purged from its recorded declarations, a recorded
//! table that is gone is reported as skipped, a member with tables but no
//! record is named as not covered, and a predicate is bound as data.

use std::sync::Arc;
use systemprompt_extension::{
    AdmittedPurge, PurgeClaim, PurgeDeclaration, PurgeFilter, SchemaDefinition, StandingTag,
};
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_users::{PurgeOutcome, UserError, UserRepository, UserService};
use uuid::Uuid;

use super::user_purge_db::core_plan;

struct Ctx {
    fixture: crate::privacy_fixture::PrivacyFixture,
    service: UserService,
    raw: Arc<sqlx::PgPool>,
    tag: String,
}

async fn setup() -> Ctx {
    let fixture = crate::privacy_fixture::PrivacyFixture::new().await;
    let raw = fixture.pool.pool();
    let service = UserService::new(Arc::new(UserRepository::new(&fixture.pool)));
    Ctx {
        fixture,
        service,
        raw,
        tag: Uuid::new_v4().simple().to_string()[..12].to_owned(),
    }
}

impl Ctx {
    fn probe_table(&self) -> String {
        format!("purge_probe_{}", self.tag)
    }

    fn probe_owner(&self) -> ExtensionId {
        ExtensionId::new(format!("probe-{}", self.tag))
    }

    async fn exec(&self, sql: &str) {
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned()))
            .execute(self.raw.as_ref())
            .await
            .expect("statement");
    }

    async fn user(&self, prefix: &str) -> UserId {
        let tag = Uuid::new_v4().simple().to_string();
        self.service
            .create(
                &format!("{prefix}-{tag}"),
                &format!("{prefix}-{tag}@purge.invalid"),
                Some("Purge Person"),
                None,
            )
            .await
            .expect("create user")
            .id
    }

    async fn probe_rows(&self, user: &UserId, kind: &str, n: usize) {
        for _ in 0..n {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {} (user_id, kind) VALUES ($1, $2)",
                self.probe_table()
            )))
            .bind(user.as_str())
            .bind(kind)
            .execute(self.raw.as_ref())
            .await
            .expect("seed probe row");
        }
    }

    async fn count(&self, user: &UserId, kind: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {} WHERE user_id = $1 AND kind = $2",
            self.probe_table()
        )))
        .bind(user.as_str())
        .bind(kind)
        .fetch_one(self.raw.as_ref())
        .await
        .expect("count")
    }

    async fn record(&self, owner: &ExtensionId, declarations: &[PurgeDeclaration]) {
        sqlx::query(
            "INSERT INTO schema_purge_records (extension_id, position, declarations) \
             VALUES ($1, 1000, $2::jsonb)",
        )
        .bind(owner.as_str())
        .bind(serde_json::to_string(declarations).expect("encode"))
        .execute(self.raw.as_ref())
        .await
        .expect("record purge declarations");
    }

    fn probe_declaration(&self, column: &str, any_of: &[&str]) -> PurgeDeclaration {
        PurgeDeclaration::Table {
            table: self.probe_table(),
            column: "user_id".to_owned(),
            predicate: Some(PurgeFilter {
                column: column.to_owned(),
                any_of: any_of.iter().map(|v| (*v).to_owned()).collect(),
            }),
        }
    }

    fn plan_with_probe(&self, declaration: &PurgeDeclaration) -> AdmittedPurge {
        let owner = self.probe_owner();
        let schemas = vec![SchemaDefinition::new(self.probe_table(), String::new())];
        let declarations = vec![declaration.clone()];
        let users = ExtensionId::new("users");
        AdmittedPurge::admit(&[
            PurgeClaim {
                owner: &users,
                standing: StandingTag::Core,
                schemas: &[],
                declarations: &[],
            },
            PurgeClaim {
                owner: &owner,
                standing: StandingTag::Install,
                schemas: &schemas,
                declarations: &declarations,
            },
        ])
        .expect("the probe member is admitted")
    }

    async fn seeded(&self) -> (UserId, UserId) {
        self.exec(&format!(
            "CREATE TABLE {} (user_id TEXT NOT NULL, kind TEXT NOT NULL)",
            self.probe_table()
        ))
        .await;
        let victim = self.user("purge-rec").await;
        let other = self.user("purge-rec-other").await;
        self.probe_rows(&victim, "user", 2).await;
        self.probe_rows(&victim, "system", 1).await;
        self.probe_rows(&other, "user", 1).await;
        (victim, other)
    }
}

fn probe_count<'a>(outcome: &'a PurgeOutcome, table: &str) -> &'a systemprompt_users::PurgeCount {
    outcome
        .counts
        .iter()
        .find(|c| c.table == table)
        .expect("the probe table is counted")
}

async fn purge_with(ctx: &Ctx, plan: &AdmittedPurge, absent: bool) {
    let (victim, other) = ctx.seeded().await;
    let declaration = ctx.probe_declaration("kind", &["user"]);
    ctx.record(&ctx.probe_owner(), &[declaration]).await;

    let preview = ctx.service.purge_preview(&victim, plan).await.expect("preview");
    let previewed = probe_count(&preview, &ctx.probe_table());
    assert_eq!(previewed.rows, 2);
    assert_eq!(previewed.absent, absent, "the preview marks a disabled member's table");
    assert_eq!(previewed.owner, ctx.probe_owner());

    let removed = ctx.service.delete(&victim, plan).await.expect("delete");
    let purged = probe_count(&removed, &ctx.probe_table());
    assert_eq!(purged.rows, 2);
    assert_eq!(purged.absent, absent);
    assert_eq!(ctx.count(&victim, "user").await, 0, "the user's own rows are gone");
    assert_eq!(
        ctx.count(&victim, "system").await,
        1,
        "rows the predicate leaves out stay"
    );
    assert_eq!(ctx.count(&other, "user").await, 1, "another user's rows stay");
    assert!(removed.skipped.is_empty());
}

#[tokio::test]
async fn a_disabled_members_user_rows_are_purged_from_its_record() {
    let ctx = setup().await;
    purge_with(&ctx, &core_plan(), true).await;
    ctx.fixture.finish().await;
}

#[tokio::test]
async fn an_enabled_member_purges_the_same_rows_from_its_admitted_declaration() {
    let ctx = setup().await;
    let plan = ctx.plan_with_probe(&ctx.probe_declaration("kind", &["user"]));
    purge_with(&ctx, &plan, false).await;
    ctx.fixture.finish().await;
}

#[tokio::test]
async fn a_recorded_table_that_is_gone_is_skipped_and_reported() {
    let ctx = setup().await;
    let victim = ctx.user("purge-gone").await;
    let gone = ExtensionId::new(format!("gone-{}", ctx.tag));
    let table = format!("gone_{}", ctx.tag);
    ctx.record(
        &gone,
        &[PurgeDeclaration::Table {
            table: table.clone(),
            column: "user_id".to_owned(),
            predicate: None,
        }],
    )
    .await;

    let preview = ctx.service.purge_preview(&victim, &core_plan()).await.expect("preview");
    assert!(!preview.is_complete());
    let removed = ctx.service.delete(&victim, &core_plan()).await.expect("delete");
    assert!(!removed.is_complete(), "a partial purge never reads as complete");
    assert!(
        removed
            .skipped
            .iter()
            .any(|s| s.owner == gone && s.table == table),
        "the missing table is reported with its owner"
    );
    ctx.fixture.finish().await;
}

#[tokio::test]
async fn a_member_with_tables_but_no_record_is_named_as_not_covered() {
    let ctx = setup().await;
    let victim = ctx.user("purge-ghost").await;
    let ghost = ExtensionId::new(format!("ghost-{}", ctx.tag));
    sqlx::query("INSERT INTO schema_table_owners (table_name, extension_id) VALUES ($1, $2)")
        .bind(format!("public.ghost_{}", ctx.tag))
        .bind(ghost.as_str())
        .execute(ctx.raw.as_ref())
        .await
        .expect("record an owner");
    let recorded = ExtensionId::new(format!("recorded-{}", ctx.tag));
    sqlx::query("INSERT INTO schema_table_owners (table_name, extension_id) VALUES ($1, $2)")
        .bind(format!("public.recorded_{}", ctx.tag))
        .bind(recorded.as_str())
        .execute(ctx.raw.as_ref())
        .await
        .expect("record an owner");
    ctx.record(&recorded, &[]).await;

    let removed = ctx.service.delete(&victim, &core_plan()).await.expect("delete");
    assert!(removed.unrecorded_members.contains(&ghost));
    assert!(
        !removed.unrecorded_members.contains(&recorded),
        "a member recorded with no declarations is vouched for"
    );
    assert!(!removed.is_complete());
    ctx.fixture.finish().await;
}

#[tokio::test]
async fn a_predicate_value_is_bound_as_data_and_cannot_widen_the_delete() {
    let ctx = setup().await;
    let (victim, other) = ctx.seeded().await;
    ctx.record(
        &ctx.probe_owner(),
        &[ctx.probe_declaration("kind", &["1=1) OR (1=1"])],
    )
    .await;
    let removed = ctx.service.delete(&victim, &core_plan()).await.expect("delete");
    assert_eq!(probe_count(&removed, &ctx.probe_table()).rows, 0);
    assert_eq!(ctx.count(&victim, "user").await, 2);
    assert_eq!(ctx.count(&victim, "system").await, 1);
    assert_eq!(ctx.count(&other, "user").await, 1);
    ctx.fixture.finish().await;
}

#[tokio::test]
async fn a_predicate_column_that_is_not_an_identifier_is_refused_before_any_delete() {
    let ctx = setup().await;
    let (victim, other) = ctx.seeded().await;
    ctx.record(
        &ctx.probe_owner(),
        &[ctx.probe_declaration("kind) OR (1=1", &["user"])],
    )
    .await;
    let refused = ctx.service.delete(&victim, &core_plan()).await;
    assert!(matches!(refused, Err(UserError::PurgeIdentifier { .. })));
    assert_eq!(ctx.count(&victim, "user").await, 2, "the transaction rolled back");
    assert_eq!(ctx.count(&other, "user").await, 1);
    ctx.fixture.finish().await;
}
