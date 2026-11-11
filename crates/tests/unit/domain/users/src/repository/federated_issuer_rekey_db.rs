//! DB-backed tests for moving federated links between issuer namespaces: a
//! move keeps each link's user, a row the target already links is removed
//! rather than moved, and a removal clears only the named issuer.

use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_users::{IssuerRekeyOutcome, IssuerRekeyPlan, UserRepository};
use uuid::Uuid;

struct Ctx {
    repo: UserRepository,
    pool: systemprompt_database::DbPool,
    own: String,
    upstream: String,
    other: String,
    tag: String,
}

async fn setup() -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let tag = Uuid::new_v4().simple().to_string();
    Ctx {
        repo: UserRepository::new(&pool),
        pool,
        own: format!("https://self-{tag}.invalid"),
        upstream: format!("https://upstream-{tag}.invalid"),
        other: format!("https://other-{tag}.invalid"),
        tag,
    }
}

impl Ctx {
    async fn user(&self, label: &str) -> UserId {
        self.repo
            .create(
                &format!("{label}-{}", self.tag),
                &format!("{label}-{}@rekey.invalid", self.tag),
                None,
                None,
            )
            .await
            .expect("create user")
            .id
    }

    async fn link(&self, issuer: &str, sub: &str, user: &UserId) {
        sqlx::query(
            "INSERT INTO federated_identities (issuer, external_sub, user_id) VALUES ($1, $2, $3)",
        )
        .bind(issuer)
        .bind(sub)
        .bind(user.as_str())
        .execute(self.pool.pool().as_ref())
        .await
        .expect("insert link");
    }

    async fn links(&self, issuer: &str) -> Vec<(String, String)> {
        let mut rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT external_sub, user_id FROM federated_identities WHERE issuer = $1",
        )
        .bind(issuer)
        .fetch_all(self.pool.pool().as_ref())
        .await
        .expect("read links");
        rows.sort();
        rows
    }
}

#[tokio::test]
async fn moving_keeps_users_drops_same_user_duplicates_and_keeps_conflicts() {
    let ctx = setup().await;
    let alice = ctx.user("alice").await;
    let bob = ctx.user("bob").await;
    let carol = ctx.user("carol").await;
    let own_slash = format!("{}/", ctx.own);
    ctx.link(&ctx.own, "sub-a", &alice).await;
    ctx.link(&own_slash, "sub-b", &bob).await;
    ctx.link(&ctx.upstream, "sub-b", &bob).await;
    ctx.link(&ctx.own, "sub-c", &alice).await;
    ctx.link(&ctx.upstream, "sub-c", &carol).await;
    ctx.link(&ctx.other, "sub-a", &carol).await;

    let plan = ctx
        .repo
        .plan_federated_issuer_rekey(&ctx.own, Some(&ctx.upstream))
        .await
        .unwrap();
    assert_eq!(
        plan,
        IssuerRekeyPlan {
            rows: 3,
            already_linked_same_user: 1,
            conflicting_users: 1,
        }
    );
    assert_eq!(
        ctx.links(&ctx.own).await.len(),
        2,
        "planning writes nothing"
    );

    let outcome = ctx
        .repo
        .move_federated_issuer(&ctx.own, &ctx.upstream)
        .await
        .unwrap();
    assert_eq!(
        outcome,
        IssuerRekeyOutcome {
            moved: 1,
            removed: 1,
            conflicts_kept: 1,
        }
    );
    assert_eq!(
        ctx.links(&ctx.own).await,
        vec![("sub-c".to_owned(), alice.as_str().to_owned())],
        "a link the target holds for another user stays where it is"
    );
    assert!(ctx.links(&own_slash).await.is_empty());
    assert_eq!(
        ctx.links(&ctx.upstream).await,
        vec![
            ("sub-a".to_owned(), alice.as_str().to_owned()),
            ("sub-b".to_owned(), bob.as_str().to_owned()),
            ("sub-c".to_owned(), carol.as_str().to_owned()),
        ]
    );
    assert_eq!(
        ctx.links(&ctx.other).await,
        vec![("sub-a".to_owned(), carol.as_str().to_owned())]
    );
}

#[tokio::test]
async fn removing_clears_only_the_named_issuer_in_both_slash_forms() {
    let ctx = setup().await;
    let alice = ctx.user("alice").await;
    ctx.link(&ctx.own, "sub-a", &alice).await;
    ctx.link(&format!("{}/", ctx.own), "sub-b", &alice).await;
    ctx.link(&ctx.upstream, "sub-a", &alice).await;

    let plan = ctx
        .repo
        .plan_federated_issuer_rekey(&ctx.own, None)
        .await
        .unwrap();
    assert_eq!(plan.rows, 2);
    let outcome = ctx.repo.remove_federated_issuer(&ctx.own).await.unwrap();
    assert_eq!(outcome.removed, 2);
    assert!(ctx.links(&ctx.own).await.is_empty());
    assert_eq!(ctx.links(&ctx.upstream).await.len(), 1);
}
