use systemprompt_extension::brain::{MemoryRecall, RecallReceipt, SourceVerdict, WithheldReason};
use systemprompt_identifiers::{AiRequestId, DocumentFolderId, UserId};

use crate::events::{Landed, price};
use crate::fakes::Access;
use crate::harness::{Harness, Setup};
use crate::recall_support::{ids, injected, principal, request};

const HINTS: &str = "hidden capped broken open prices";

struct Docs {
    folder: String,
    owner: UserId,
    hidden: Landed,
    capped: Landed,
    broken: Landed,
    open: Landed,
}

async fn capture(h: &Harness) -> Docs {
    let owner = h.user("owner", &[]).await;
    let folder = DocumentFolderId::generate();
    let docs = Docs {
        folder: folder.as_str().to_owned(),
        owner: owner.clone(),
        hidden: Landed::new().in_folder(folder.clone()),
        capped: Landed::new().in_folder(folder.clone()),
        broken: Landed::new().in_folder(folder.clone()),
        open: Landed::new().in_folder(folder),
    };
    for (doc, product) in [
        (&docs.hidden, "hidden"),
        (&docs.capped, "capped"),
        (&docs.broken, "broken"),
        (&docs.open, "open"),
    ] {
        h.model
            .push(serde_json::json!([price(product, "emea", "1200")]));
        h.emit(&doc.event(&owner)).await;
    }
    h.drain().await;
    docs
}

#[tokio::test]
async fn an_intern_without_finance_access_is_withheld_every_fact_whose_source_they_cannot_read() {
    let h = Harness::with("brain_withheld_source", Setup::confirming()).await;
    let docs = capture(&h).await;
    let intern = h.user("intern", &[]).await;
    h.access.set(
        docs.hidden.document.as_str(),
        &intern,
        Access::Verdict(SourceVerdict::Hidden),
    );
    h.access.set(
        docs.capped.document.as_str(),
        &intern,
        Access::Verdict(SourceVerdict::AgentCapped),
    );
    h.access
        .set(docs.broken.document.as_str(), &intern, Access::Error);
    let service = h.recall();

    let owner_view = service
        .recall(&request(principal(&docs.owner, Some(&docs.folder)), HINTS))
        .await
        .expect("owner recall");
    assert_eq!(
        ids(&owner_view).len(),
        4,
        "negative control: a reader of every source sees all four"
    );

    let req = request(principal(&intern, Some(&docs.folder)), HINTS);
    let outcome = service.recall(&req).await.expect("intern recall");
    let block = injected(&outcome)
        .expect("the readable fact is injected")
        .clone();

    assert_eq!(ids(&outcome), h.fact_ids("open").await);
    assert_eq!(
        block.withheld.by_reason.get(&WithheldReason::Source),
        Some(&2)
    );
    assert_eq!(
        block.withheld.by_reason.get(&WithheldReason::AgentCapped),
        Some(&1)
    );
    assert!(h.access.calls() >= 8, "every source was checked");

    service
        .record(&RecallReceipt::new(&req, AiRequestId::generate(), outcome))
        .await
        .expect("record");
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT withheld FROM brain_recalls WHERE principal_user_id = $1")
            .bind(intern.as_str())
            .fetch_one(&h.pg)
            .await
            .expect("recall row");
    assert_eq!(
        stored,
        serde_json::to_value(&block.withheld.by_reason).expect("encode")
    );
    h.finish().await;
}

#[tokio::test]
async fn a_subject_whose_every_fact_is_hidden_from_the_reader_names_nothing() {
    let h = Harness::with("brain_withheld_hidden_subject", Setup::confirming()).await;
    let docs = capture(&h).await;
    let mut declared = crate::plane::declared();
    declared.retrieval = Some(systemprompt_models::brain::RetrievalDoc {
        context_requires_signal: true,
        ..systemprompt_models::brain::RetrievalDoc::default()
    });
    crate::plane::apply(&h.pg, &declared).await;
    let intern = h.user("intern", &[]).await;
    h.access.set(
        docs.hidden.document.as_str(),
        &intern,
        Access::Verdict(SourceVerdict::Hidden),
    );
    let service = h.recall();

    let about_hidden = service
        .recall(&request(
            principal(&intern, Some(&docs.folder)),
            "Any news on hidden?",
        ))
        .await
        .expect("intern recall");
    assert!(
        ids(&about_hidden).is_empty(),
        "a subject the reader cannot see opened the context cap: {:?}",
        ids(&about_hidden)
    );

    let about_open = service
        .recall(&request(
            principal(&intern, Some(&docs.folder)),
            "Any news on open?",
        ))
        .await
        .expect("intern recall");
    let got = ids(&about_open);
    assert_eq!(got.len(), 3, "a visible subject opens the context: {got:?}");
    assert!(got.contains(&h.fact_ids("open").await[0]), "{got:?}");
    h.finish().await;
}
