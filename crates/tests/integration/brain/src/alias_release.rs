//! An erased or retired fact holds no alias: ownership of a shared alias
//! passes to the newest live fact that still contributes it, a later
//! candidate naming the alias resolves to that fact's subject, and an
//! alias no live fact contributes is gone.

use chrono::Utc;
use systemprompt_brain::FactRepository;
use systemprompt_brain::models::FactChange;
use systemprompt_models::brain::{EnteredFact, EnteredStatus, FactScope, SubjectKey};

use crate::fact_index::{CONTACT, declared_with_contact};
use crate::harness::{Harness, Setup};
use crate::plane::{self, kind};

fn contact(subject: &str, name: &str) -> EnteredFact {
    EnteredFact {
        kind: kind(CONTACT),
        subject: SubjectKey::new("client", subject),
        fields: serde_json::Map::from_iter([
            ("name".to_owned(), serde_json::json!(name)),
            (
                "account".to_owned(),
                serde_json::json!(format!("{subject}-holdings")),
            ),
            ("owner".to_owned(), serde_json::json!("user:u_42")),
        ]),
        scope: FactScope::Org,
        topics: Vec::new(),
        status: EnteredStatus::Proposed,
        observed_at: None,
    }
}

async fn owner_of(h: &Harness, norm: &str) -> Option<(String, Option<String>)> {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT subject_id, owner_fact_id FROM brain_aliases
         WHERE subject_kind = 'client' AND alias_norm = $1",
    )
    .bind(norm)
    .fetch_optional(&h.pg)
    .await
    .expect("alias row")
}

#[tokio::test]
async fn a_released_alias_passes_to_the_newest_live_contributor_then_disappears() {
    let h = Harness::with("brain_alias_release", Setup::confirming()).await;
    plane::apply(&h.pg, &declared_with_contact()).await;
    let author = h.user("author", &[]).await;
    let contacts = [
        ("acme-rockets", "Jane Doe"),
        ("globex", "jane doe"),
        ("initech", "Jane  Doe"),
    ];
    for (subject, name) in contacts {
        let event = contact(subject, name)
            .to_event(author.clone(), Utc::now())
            .expect("entered event");
        h.emit(&event).await;
        h.drain().await;
    }
    let initech = h.only_fact("initech").await;
    let globex = h.only_fact("globex").await;
    let acme = h.only_fact("acme-rockets").await;
    assert_eq!(
        owner_of(&h, "jane doe").await,
        Some(("initech".to_owned(), Some(initech.id.to_string()))),
        "the newest contributor owns the shared alias"
    );

    h.service
        .erase(&Harness::caller(&author, &["admin"]), &initech.id)
        .await
        .expect("erase the owner");
    assert_eq!(
        owner_of(&h, "jane doe").await,
        Some(("globex".to_owned(), Some(globex.id.to_string()))),
        "the next newest live contributor takes it"
    );

    let mut conn = h.pg.acquire().await.expect("conn");
    FactRepository::apply(
        &mut conn,
        &globex.id,
        FactChange::Retracted {
            actor: author.clone(),
        },
    )
    .await
    .expect("retire the new owner");
    drop(conn);
    assert_eq!(
        owner_of(&h, "jane doe").await,
        Some(("acme-rockets".to_owned(), Some(acme.id.to_string()))),
        "a retired fact holds no alias either"
    );

    let event = contact("Jane Doe", "Jane Doe")
        .to_event(author.clone(), Utc::now())
        .expect("entered event");
    h.emit(&event).await;
    h.drain().await;
    assert!(
        h.fact_ids("Jane Doe").await.is_empty(),
        "a candidate naming the alias resolves to the surviving subject"
    );
    assert_eq!(h.fact_ids("acme-rockets").await.len(), 1);

    h.service
        .erase(&Harness::caller(&author, &["admin"]), &acme.id)
        .await
        .expect("erase the last contributor");
    assert_eq!(
        owner_of(&h, "jane doe").await,
        None,
        "an alias no live fact contributes is gone"
    );
    h.finish().await;
}
