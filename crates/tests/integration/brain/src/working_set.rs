//! The working-set projection: a stored fact has one row per scope key it
//! is reachable through, rows follow a revision, a topic audience change
//! rebuilds them, a delete cascades them, and the sweep rebuilds a
//! projection whose count has drifted.

use systemprompt_brain::plane::Rebuilds;
use systemprompt_brain::repository::WorkingSetRepository;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::FactScope;
use systemprompt_models::notifications::AudienceSubject;

use crate::events::{Landed, entered, price};
use crate::harness::{Harness, Setup};
use crate::plane::{self, ENGINEERING, FINANCE, OWNERSHIP, kind};

async fn keys(h: &Harness, id: &BrainFactId) -> Vec<String> {
    WorkingSetRepository::keys_for(&h.pg, id)
        .await
        .expect("working set keys")
}

#[tokio::test]
async fn a_persisted_fact_has_a_row_for_its_scope_and_each_topic_audience() {
    let h = Harness::with("brain_ws_persist", Setup::confirming()).await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.emit(&entered(
        &author,
        kind(OWNERSHIP),
        "globex",
        serde_json::json!({"team": "ops"}),
        FactScope::User(author.clone()),
    ))
    .await;
    h.drain().await;

    let acme = h.only_fact("acme").await;
    assert_eq!(acme.scope, FactScope::User(author.clone()));
    assert_eq!(
        keys(&h, &acme.id).await,
        vec!["org".to_owned(), format!("user:{author}")],
        "the pricing kind carries the finance topic, whose audience is everyone"
    );
    let globex = h.only_fact("globex").await;
    assert_eq!(
        keys(&h, &globex.id).await,
        vec!["org".to_owned(), format!("user:{author}")]
    );
    let counts = WorkingSetRepository::counts(&h.pg).await.expect("counts");
    assert!(counts.is_consistent(), "{counts:?}");
    assert_eq!(counts.stored, 4);
    h.finish().await;
}

#[tokio::test]
async fn a_topic_audience_change_rebuilds_the_set_and_a_delete_cascades_it() {
    let h = Harness::with("brain_ws_rebuild", Setup::confirming()).await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    let acme = h.only_fact("acme").await;

    let mut declared = plane::declared();
    let finance = declared
        .topics
        .iter_mut()
        .find(|t| t.id.as_str() == FINANCE)
        .expect("finance topic");
    finance.audience = vec![
        AudienceSubject::Role("finance_lead".to_owned()),
        AudienceSubject::Dimension {
            kind: "group".to_owned(),
            value: "emea".to_owned(),
        },
    ];
    plane::apply(&h.pg, &declared).await;
    assert_eq!(
        keys(&h, &acme.id).await,
        vec![
            "group:emea".to_owned(),
            "role:finance_lead".to_owned(),
            format!("user:{author}"),
        ],
        "everyone is gone and the two declared audiences are in"
    );
    assert_eq!(
        Rebuilds::for_keys([format!("topic/{ENGINEERING}").as_str()]),
        Rebuilds {
            working_set: true,
            links: false,
            search: false,
        }
    );

    let caller = Harness::caller(&author, &[]);
    h.service.erase(&caller, &acme.id).await.expect("erase");
    assert!(
        keys(&h, &acme.id).await.is_empty(),
        "rows cascade with the fact"
    );
    assert_eq!(h.total("brain_fact_links").await, 0, "links cascade too");
    h.finish().await;
}

#[tokio::test]
async fn the_sweep_rebuilds_a_drifted_projection() {
    let h = Harness::with("brain_ws_sweep", Setup::confirming()).await;
    let author = h.user("author", &[]).await;
    h.model.push(serde_json::json!([
        price("acme", "emea", "1200"),
        price("globex", "emea", "700"),
    ]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    sqlx::query("DELETE FROM brain_working_set WHERE scope_key = 'org'")
        .execute(&h.pg)
        .await
        .expect("drift");
    let before = WorkingSetRepository::counts(&h.pg).await.expect("counts");
    assert!(!before.is_consistent(), "{before:?}");

    let report = h
        .pipeline
        .sweep(chrono::Utc::now())
        .await
        .expect("sweep")
        .expect("this replica swept");
    assert!(report.working_set_rebuilt);
    assert_eq!(report.working_set, before);
    let after = WorkingSetRepository::counts(&h.pg).await.expect("counts");
    assert!(after.is_consistent(), "{after:?}");
    assert_eq!(after.stored, 4);

    let mut conn = h.pg.acquire().await.expect("conn");
    let written = WorkingSetRepository::rebuild_all(&mut conn)
        .await
        .expect("rebuild");
    assert_eq!(written, 4, "rebuild_all answers the rows it wrote");
    h.finish().await;
}
