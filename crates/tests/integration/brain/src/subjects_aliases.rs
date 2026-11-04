use chrono::{DateTime, Duration, Utc};
use sqlx::PgConnection;
use systemprompt_brain::index::{AliasOwner, AliasRows};
use systemprompt_brain::repository::{AliasRepository, SubjectRepository};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::SubjectKey;

use crate::harness::Harness;

fn owned(subject: &SubjectKey, alias: &str, fact: &BrainFactId, at: DateTime<Utc>) -> AliasRows {
    AliasRows::collect([(
        AliasOwner {
            subject,
            fact,
            updated_at: at,
        },
        vec![alias.to_owned()],
    )])
}

async fn display_name(h: &Harness, subject: &SubjectKey) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT display_name FROM brain_subjects WHERE subject_kind = $1 AND subject_id = $2",
    )
    .bind(&subject.kind)
    .bind(&subject.id)
    .fetch_one(&h.pg)
    .await
    .expect("display name")
}

#[test]
fn normalise_collapses_whitespace_and_case() {
    assert_eq!(
        AliasRepository::normalise("  ACME   Corp\tLtd "),
        "acme corp ltd"
    );
    assert_eq!(AliasRepository::normalise(""), "");
}

#[tokio::test]
async fn ensure_creates_a_subject_once_and_keeps_its_display_name() {
    let h = Harness::new("brain_subjects_ensure").await;
    let acme = SubjectKey::new("product", "acme");
    assert!(!h.repos.subjects.exists(&acme).await.expect("exists"));

    let mut conn = h.pg.acquire().await.expect("conn");
    SubjectRepository::ensure(&mut conn, &acme, Some("Acme"))
        .await
        .expect("ensure");
    assert!(h.repos.subjects.exists(&acme).await.expect("exists"));
    assert_eq!(display_name(&h, &acme).await.as_deref(), Some("Acme"));

    SubjectRepository::ensure(&mut conn, &acme, None)
        .await
        .expect("ensure again");
    assert_eq!(display_name(&h, &acme).await.as_deref(), Some("Acme"));
    SubjectRepository::ensure(&mut conn, &acme, Some("Acme Corp"))
        .await
        .expect("rename");
    assert_eq!(display_name(&h, &acme).await.as_deref(), Some("Acme Corp"));
    drop(conn);
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_subjects WHERE subject_id = $1",
            "acme"
        )
        .await,
        1
    );
    assert!(
        !h.repos
            .subjects
            .exists(&SubjectKey::new("customer", "acme"))
            .await
            .expect("other kind")
    );
    h.finish().await;
}

#[tokio::test]
async fn aliases_match_by_similarity_within_their_kind_and_move_on_upsert() {
    let h = Harness::new("brain_subjects_alias").await;
    let acme = SubjectKey::new("product", "acme");
    let globex = SubjectKey::new("product", "globex");
    let mut conn = h.pg.acquire().await.expect("conn");
    for subject in [&acme, &globex] {
        SubjectRepository::ensure(&mut conn, subject, None)
            .await
            .expect("subject");
    }
    let (first, second, third) = (
        BrainFactId::generate(),
        BrainFactId::generate(),
        BrainFactId::generate(),
    );
    let at = Utc::now();
    AliasRepository::upsert_many(&mut conn, &owned(&acme, "Acme Rockets", &first, at))
        .await
        .expect("alias");
    AliasRepository::upsert_many(
        &mut conn,
        &owned(&globex, "Globex Corporation", &second, at),
    )
    .await
    .expect("alias");

    let found = h
        .repos
        .aliases
        .similar_to("product", "acme  ROCKETS", 0.3)
        .await
        .expect("similar");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].subject, acme);
    assert_eq!(found[0].alias, "Acme Rockets");
    assert!((found[0].similarity - 1.0).abs() < 1e-6, "{found:?}");

    let near = h
        .repos
        .aliases
        .similar_to("product", "acme rocket", 0.3)
        .await
        .expect("near");
    assert_eq!(near.first().map(|m| &m.subject), Some(&acme));
    assert!(near[0].similarity < 1.0);

    assert!(
        h.repos
            .aliases
            .similar_to("customer", "acme rockets", 0.3)
            .await
            .expect("other kind")
            .is_empty()
    );
    assert!(
        h.repos
            .aliases
            .similar_to("product", "zzzz qqqq", 0.3)
            .await
            .expect("unrelated")
            .is_empty()
    );

    let later = at + Duration::seconds(1);
    AliasRepository::upsert_many(&mut conn, &owned(&globex, "ACME rockets", &third, later))
        .await
        .expect("move alias");
    drop(conn);
    let moved = h
        .repos
        .aliases
        .similar_to("product", "acme rockets", 0.9)
        .await
        .expect("moved");
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert_eq!(moved[0].subject, globex);
    assert_eq!(moved[0].alias, "ACME rockets");
    assert_eq!(h.total("brain_aliases").await, 2);
    h.finish().await;
}

async fn alias_owner(conn: &mut PgConnection, norm: &str) -> (String, Option<String>) {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT subject_id, owner_fact_id FROM brain_aliases
         WHERE subject_kind = 'product' AND alias_norm = $1",
    )
    .bind(norm)
    .fetch_one(conn)
    .await
    .expect("alias row")
}

#[tokio::test]
async fn a_set_upsert_moves_an_alias_only_to_a_fact_at_least_as_recent_as_its_owner() {
    let h = Harness::new("brain_alias_owner_rule").await;
    let acme = SubjectKey::new("product", "acme");
    let globex = SubjectKey::new("product", "globex");
    let mut conn = h.pg.acquire().await.expect("conn");
    for subject in [&acme, &globex] {
        SubjectRepository::ensure(&mut conn, subject, None)
            .await
            .expect("subject");
    }
    let mut ids = vec![BrainFactId::generate(), BrainFactId::generate()];
    ids.sort();
    let (lower, higher) = (ids[0].clone(), ids[1].clone());
    let newest = Utc::now();
    let older = newest - Duration::seconds(5);
    let upsert = |owner: AliasOwner<'_>| AliasRows::collect([(owner, vec!["Jane Doe".to_owned()])]);

    let first = upsert(AliasOwner {
        subject: &acme,
        fact: &higher,
        updated_at: newest,
    });
    AliasRepository::upsert_many(&mut conn, &first)
        .await
        .expect("first owner");
    assert_eq!(
        alias_owner(&mut conn, "jane doe").await,
        ("acme".to_owned(), Some(higher.to_string()))
    );

    let stale = upsert(AliasOwner {
        subject: &globex,
        fact: &lower,
        updated_at: older,
    });
    AliasRepository::upsert_many(&mut conn, &stale)
        .await
        .expect("an older fact");
    assert_eq!(
        alias_owner(&mut conn, "jane doe").await,
        ("acme".to_owned(), Some(higher.to_string())),
        "an older fact cannot take the alias"
    );

    let tied_lower = upsert(AliasOwner {
        subject: &globex,
        fact: &lower,
        updated_at: newest,
    });
    AliasRepository::upsert_many(&mut conn, &tied_lower)
        .await
        .expect("a tied stamp with the lower id");
    assert_eq!(
        alias_owner(&mut conn, "jane doe").await,
        ("acme".to_owned(), Some(higher.to_string())),
        "on a tied stamp the greater id keeps it"
    );

    let newer = upsert(AliasOwner {
        subject: &globex,
        fact: &lower,
        updated_at: newest + Duration::seconds(1),
    });
    AliasRepository::upsert_many(&mut conn, &newer)
        .await
        .expect("a newer fact");
    assert_eq!(
        alias_owner(&mut conn, "jane doe").await,
        ("globex".to_owned(), Some(lower.to_string())),
        "a newer fact takes it whatever its id"
    );

    drop(conn);
    h.finish().await;
}
