use systemprompt_brain::models::FactFilter;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::brain::FactStatus;

use crate::events::{Landed, price};
use crate::harness::Harness;
use crate::plane::{OWNERSHIP, PRICING, kind};

async fn seeded(h: &Harness) -> (UserId, Vec<BrainFactId>) {
    let owner = h.user("owner", &[]).await;
    h.model.push(serde_json::json!([
        price("acme", "emea", "1200"),
        price("globex", "emea", "700"),
    ]));
    h.emit(&Landed::new().event(&owner)).await;
    h.drain().await;
    let mut ids = h.fact_ids("acme").await;
    ids.extend(h.fact_ids("globex").await);
    assert_eq!(ids.len(), 2, "{ids:?}");
    (owner, ids)
}

fn sorted(mut ids: Vec<BrainFactId>) -> Vec<BrainFactId> {
    ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    ids
}

#[tokio::test]
async fn search_narrows_by_subject_kind_status_and_limit() {
    let h = Harness::new("brain_search_filter").await;
    let (owner, ids) = seeded(&h).await;
    let caller = Harness::caller(&owner, &[]);

    let all = h
        .service
        .search(&caller, &FactFilter::new())
        .await
        .expect("all");
    assert_eq!(
        sorted(all.iter().map(|f| f.id.clone()).collect()),
        sorted(ids.clone())
    );

    let acme_subject = h.fact(&h.fact_ids("acme").await[0]).await.subject;
    let acme = h
        .service
        .search(&caller, &FactFilter::new().with_subject(acme_subject))
        .await
        .expect("by subject");
    assert_eq!(acme.len(), 1);
    assert!(acme.iter().all(|f| f.subject.id == "acme"), "{acme:?}");

    let pricing = h
        .service
        .search(&caller, &FactFilter::new().with_kind(kind(PRICING)))
        .await
        .expect("by kind");
    assert_eq!(pricing.len(), 2);
    let ownership = h
        .service
        .search(&caller, &FactFilter::new().with_kind(kind(OWNERSHIP)))
        .await
        .expect("other kind");
    assert!(ownership.is_empty(), "{ownership:?}");

    let fresh = h.fact(&ids[1]).await.status;
    assert_ne!(fresh, FactStatus::Confirmed);
    h.service.confirm(&caller, &ids[0]).await.expect("confirm");
    let confirmed = h
        .service
        .search(
            &caller,
            &FactFilter::new().with_statuses(vec![FactStatus::Confirmed]),
        )
        .await
        .expect("by status");
    assert_eq!(
        confirmed.iter().map(|f| f.id.clone()).collect::<Vec<_>>(),
        vec![ids[0].clone()]
    );
    let others = h
        .service
        .search(
            &caller,
            &FactFilter::new().with_statuses(vec![fresh, FactStatus::Retired]),
        )
        .await
        .expect("other statuses");
    assert_eq!(others.len(), 1);
    assert!(others.iter().all(|f| f.id != ids[0]));

    let limited = h
        .service
        .search(&caller, &FactFilter::new().with_limit(1))
        .await
        .expect("limited");
    assert_eq!(limited.len(), 1);
    h.finish().await;
}
