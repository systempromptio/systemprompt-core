use systemprompt_brain::repository::{AliasRepository, SubjectRepository};
use systemprompt_models::brain::SubjectKey;

use crate::harness::Harness;

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
    AliasRepository::upsert(&mut conn, &acme, "Acme Rockets")
        .await
        .expect("alias");
    AliasRepository::upsert(&mut conn, &globex, "Globex Corporation")
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

    AliasRepository::upsert(&mut conn, &globex, "ACME rockets")
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
