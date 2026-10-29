use chrono::Utc;
use systemprompt_identifiers::DocumentId;
use systemprompt_models::notifications::{
    Attention, AttentionReason, AudienceSubject, RuleAction, RuleMatch, SimulateRequest,
};

use crate::harness::{Harness, mention};

#[tokio::test]
async fn simulation_answers_what_processing_then_does() {
    let h = Harness::new("notif_simulate").await;
    let author = h.user("author", &[]).await;
    let quiet = h.user("quiet", &["ops"]).await;
    let loud = h.user("loud", &[]).await;
    let hidden = h.user("hidden", &[]).await;
    let rule = h
        .declare_rule(
            RuleMatch {
                recipients: vec![AudienceSubject::Role("ops".to_owned())],
                ..RuleMatch::default()
            },
            RuleAction {
                cap: Some(Attention::Silent),
                ..RuleAction::default()
            },
        )
        .await;
    h.hidden.hide_from(&hidden);
    let (event, _) = mention(&author, &quiet, &DocumentId::generate(), Utc::now());
    let event = event
        .with_direct_recipient(loud.clone(), None)
        .with_direct_recipient(hidden.clone(), None)
        .with_direct_recipient(author.clone(), None);

    let simulated = h
        .service
        .simulate(SimulateRequest {
            event: event.clone(),
        })
        .await
        .expect("simulate");
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM notifications WHERE kind = $1",
            "document.comment.mentioned"
        )
        .await,
        0,
        "simulation writes nothing"
    );
    assert!(
        simulated.decisions.iter().all(|d| d.user != author),
        "the actor is never planned"
    );
    let decision = |user: &systemprompt_identifiers::UserId| {
        simulated
            .decisions
            .iter()
            .find(|d| &d.user == user)
            .cloned()
            .expect("a decision per recipient")
    };
    let quiet_decision = decision(&quiet);
    assert_eq!(quiet_decision.attention, Some(Attention::Silent));
    assert_eq!(quiet_decision.reason, AttentionReason::Rule(rule.clone()));
    assert_eq!(quiet_decision.matched_rules, vec![rule]);
    assert_eq!(decision(&loud).attention, Some(Attention::Human));
    let hidden_decision = decision(&hidden);
    assert_eq!(hidden_decision.attention, None);
    assert!(hidden_decision.withheld_reason.is_some());

    h.emit(&event).await;
    h.drain().await;
    for planned in &simulated.decisions {
        let rows = h.rows_for(&planned.user).await;
        match planned.attention {
            Some(attention) => {
                assert_eq!(rows.len(), 1, "{:?}", planned.user);
                assert_eq!(rows[0].attention, attention);
                assert_eq!(rows[0].attention_reason, planned.reason);
            },
            None => assert!(rows.is_empty(), "{:?}", planned.user),
        }
    }
    h.finish().await;
}
