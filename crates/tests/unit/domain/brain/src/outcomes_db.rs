//! DB-backed: response citations and corrections recorded against a recall
//! in the test database, idempotent per recall, fact and signal.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use systemprompt_brain::outcomes::OutcomeService;
use systemprompt_brain::repository::OutcomeRepository;
use systemprompt_brain::{BrainRepositories, NewRecall};
use systemprompt_extension::brain::RecallConversation;
use systemprompt_identifiers::{AiRequestId, BrainFactId, BrainRecallId};
use systemprompt_models::brain::{FactRef, RecallAudit};
use systemprompt_models::origin::ClientKind;
use systemprompt_test_fixtures::credential::seed_user_row;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_test_fixtures::user::unique_user_id;

#[tokio::test]
async fn citations_and_corrections_are_recorded_once() {
    let pool = test_db_pool().await;
    let repos = BrainRepositories::new(&pool);
    let user = unique_user_id("brain-outcomes");
    seed_user_row(&pool, &user, &format!("{}@example.test", user.as_str()))
        .await
        .expect("user");

    let cited = BrainFactId::generate();
    let corrected = BrainFactId::generate();
    let ai_request_id = AiRequestId::generate();
    let audit = RecallAudit {
        recall_id: BrainRecallId::generate(),
        injected: vec![
            FactRef {
                id: cited.clone(),
                revision: 1,
            },
            FactRef {
                id: corrected.clone(),
                revision: 2,
            },
        ],
        withheld: BTreeMap::new(),
        tokens: 40,
        as_of: Utc::now(),
    };
    let recall = NewRecall::builder(user.clone(), ClientKind::ClaudeCode, audit)
        .conversation(RecallConversation {
            ai_request_id: Some(ai_request_id.clone()),
            ..RecallConversation::default()
        })
        .project("outcomes-test")
        .rendered_block("<team_memory/>")
        .build();
    repos
        .recalls
        .record_detached(&recall)
        .await
        .expect("recall");

    let service = OutcomeService::new(
        std::sync::Arc::clone(&repos.recalls),
        std::sync::Arc::clone(&repos.outcomes),
        Vec::new(),
    );
    let text = format!("Per {cited}, deploys go through the gate.");
    assert_eq!(
        service
            .observe_response(&ai_request_id, &text)
            .await
            .expect("observe"),
        1
    );
    assert_eq!(
        service
            .observe_response(&ai_request_id, &text)
            .await
            .expect("replay"),
        0
    );
    assert_eq!(
        service
            .observe_response(&AiRequestId::generate(), &text)
            .await
            .expect("no recall"),
        0
    );

    let now = Utc::now();
    let since = now - Duration::days(14);
    let first = OutcomeRepository::record_corrections(repos.write_pool(), &corrected, since, now)
        .await
        .expect("correct");
    assert_eq!(first, 1);
    let again = OutcomeRepository::record_corrections(repos.write_pool(), &corrected, since, now)
        .await
        .expect("again");
    assert_eq!(again, 0);
    let outside = OutcomeRepository::record_corrections(
        repos.write_pool(),
        &cited,
        now + Duration::days(1),
        now,
    )
    .await
    .expect("outside window");
    assert_eq!(outside, 0);

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.as_str())
        .execute(repos.write_pool())
        .await
        .expect("cleanup");
}
