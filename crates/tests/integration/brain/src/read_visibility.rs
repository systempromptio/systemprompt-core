use systemprompt_brain::BrainError;
use systemprompt_brain::models::FactFilter;
use systemprompt_extension::brain::SourceVerdict;
use systemprompt_identifiers::DocumentFolderId;
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{EnteredFact, EnteredStatus, FactScope, SubjectKey};

use crate::events::{Landed, price};
use crate::fakes::Access;
use crate::harness::{Harness, Setup};
use crate::plane::{OWNERSHIP, kind};

#[tokio::test]
async fn a_fact_from_a_document_the_reader_cannot_open_is_neither_listed_nor_readable() {
    let h = Harness::with("brain_read_visibility", Setup::confirming()).await;
    let owner = h.user("owner", &[]).await;
    let intern = h.user("intern", &[]).await;
    let folder = DocumentFolderId::generate();
    let hidden = Landed::new().in_folder(folder.clone());
    let open = Landed::new().in_folder(folder.clone());
    for (doc, product) in [(&hidden, "hidden"), (&open, "open")] {
        h.model
            .push(serde_json::json!([price(product, "emea", "1200")]));
        h.emit(&doc.event(&owner)).await;
    }
    h.drain().await;
    h.access.set(
        hidden.document.as_str(),
        &intern,
        Access::Verdict(SourceVerdict::Hidden),
    );
    let hidden_id = h.fact_ids("hidden").await.remove(0);
    let open_id = h.fact_ids("open").await.remove(0);
    let as_reader = |user| {
        Harness::caller(user, &[])
            .with_actor(RequestActor::Human)
            .with_project(Some(folder.as_str().to_owned()))
    };

    let owner_sees = h
        .service
        .search(&as_reader(&owner), &FactFilter::new())
        .await
        .expect("owner search");
    assert_eq!(
        owner_sees.len(),
        2,
        "negative control: the owner reads both sources"
    );

    let intern_sees: Vec<_> = h
        .service
        .search(&as_reader(&intern), &FactFilter::new())
        .await
        .expect("intern search")
        .into_iter()
        .map(|fact| fact.id)
        .collect();
    assert_eq!(intern_sees, vec![open_id.clone()]);
    let read = h.service.get(&as_reader(&intern), &hidden_id).await;
    assert!(matches!(read, Err(BrainError::NotFound(_))), "{read:?}");
    assert!(h.service.get(&as_reader(&intern), &open_id).await.is_ok());
    h.finish().await;
}

#[tokio::test]
async fn a_stranger_may_not_enter_an_org_fact() {
    let h = Harness::new("brain_enter_authority").await;
    let stranger = h.user("stranger", &[]).await;
    let admin = h.user("admin", &["admin"]).await;
    let fact = EnteredFact {
        kind: kind(OWNERSHIP),
        subject: SubjectKey::new("product", "acme"),
        fields: serde_json::Map::from_iter([("team".to_owned(), serde_json::json!("ops"))]),
        scope: FactScope::Org,
        topics: Vec::new(),
        status: EnteredStatus::Candidate,
    };
    let refused = h
        .service
        .enter(&Harness::caller(&stranger, &[]), &fact)
        .await
        .expect_err("a stranger is refused");
    assert!(
        matches!(refused, BrainError::Forbidden { .. }),
        "{refused:?}"
    );
    h.service
        .enter(&Harness::caller(&admin, &["admin"]), &fact)
        .await
        .expect("negative control: an admin enters it");
    h.finish().await;
}
