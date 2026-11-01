use chrono::Duration;
use systemprompt_brain::models::EventOutcome;
use systemprompt_brain::pipeline::persist::Effect;
use systemprompt_brain::pipeline::rules::{
    InterestProbe, add_kind_topics, interest_matches, matching_interests,
};
use systemprompt_brain::pipeline::{CandidateResult, EventSummary, Persisted, Tally};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{InterestDecl, Pathway};

use crate::fixtures::{at, candidate, kind, topic};
use crate::pipeline_fixtures::kind_decl;

fn interest(name: &str) -> InterestDecl {
    serde_json::from_value(serde_json::json!({
        "id": name,
        "name": name,
        "owner": "owner-1",
        "question": "what changed?",
    }))
    .expect("interest")
}

fn probe<'a>(
    candidate: &'a systemprompt_models::brain::FactCandidate,
    statement: &'a str,
) -> InterestProbe<'a> {
    InterestProbe {
        candidate,
        statement,
        pathway: Pathway::DocumentRevision,
        now: at(1_000_000),
    }
}

#[test]
fn an_unconstrained_interest_matches_any_candidate() {
    let fact = candidate("pricing", "client:acme");
    assert!(interest_matches(&interest("any"), &probe(&fact, "x")));
}

#[test]
fn an_expired_interest_never_matches() {
    let fact = candidate("pricing", "client:acme");
    let mut expired = interest("expired");
    expired.expires = Some(at(1_000_000));
    assert!(!interest_matches(&expired, &probe(&fact, "x")));
    expired.expires = Some(at(1_000_000) + Duration::seconds(1));
    assert!(interest_matches(&expired, &probe(&fact, "x")));
}

#[test]
fn each_match_dimension_narrows_the_interest() {
    let fact = candidate("pricing", "client:acme");
    let statement = "Acme pays the ENTERPRISE rate";

    let mut by_pathway = interest("pathway");
    by_pathway.pathways = vec![Pathway::CommentResolved];
    assert!(!interest_matches(&by_pathway, &probe(&fact, statement)));
    by_pathway.pathways.push(Pathway::DocumentRevision);
    assert!(interest_matches(&by_pathway, &probe(&fact, statement)));

    let mut by_kind = interest("kind");
    by_kind.r#match.kinds = vec![kind("commitment")];
    assert!(!interest_matches(&by_kind, &probe(&fact, statement)));
    by_kind.r#match.kinds.push(kind("pricing"));
    assert!(interest_matches(&by_kind, &probe(&fact, statement)));

    let mut by_topic = interest("topic");
    by_topic.r#match.topics = vec![topic("legal")];
    assert!(!interest_matches(&by_topic, &probe(&fact, statement)));
    by_topic.r#match.topics = vec![topic("ops")];
    assert!(interest_matches(&by_topic, &probe(&fact, statement)));

    let mut by_subject = interest("subject");
    by_subject.r#match.subjects = vec!["client:globex".to_owned()];
    assert!(!interest_matches(&by_subject, &probe(&fact, statement)));
    by_subject.r#match.subjects = vec!["client:acme".to_owned()];
    assert!(interest_matches(&by_subject, &probe(&fact, statement)));
    by_subject.r#match.subjects = vec!["acme".to_owned()];
    assert!(interest_matches(&by_subject, &probe(&fact, statement)));

    let mut by_keyword = interest("keyword");
    by_keyword.r#match.keywords = vec!["discount".to_owned()];
    assert!(!interest_matches(&by_keyword, &probe(&fact, statement)));
    by_keyword.r#match.keywords = vec!["Enterprise".to_owned()];
    assert!(interest_matches(&by_keyword, &probe(&fact, statement)));
}

#[test]
fn matching_interests_keeps_only_the_matching_declarations_in_order() {
    let fact = candidate("pricing", "client:acme");
    let mut other_kind = interest("other-kind");
    other_kind.r#match.kinds = vec![kind("commitment")];
    let declared = vec![interest("first"), other_kind, interest("last")];

    let matched: Vec<&str> = matching_interests(&declared, &probe(&fact, "x"))
        .into_iter()
        .map(|i| i.name.as_str())
        .collect();

    assert_eq!(matched, vec!["first", "last"]);
    assert!(matching_interests(&[], &probe(&fact, "x")).is_empty());
}

#[test]
fn kind_topics_are_added_once_without_reordering_existing_ones() {
    let mut fact = candidate("commitment", "client:acme");
    let mut decl = kind_decl();
    decl.topics = vec![topic("ops"), topic("finance"), topic("legal")];

    add_kind_topics(&mut fact, &decl);
    add_kind_topics(&mut fact, &decl);

    assert_eq!(
        fact.topics,
        vec![topic("ops"), topic("finance"), topic("legal")]
    );
}

#[test]
fn a_tally_counts_each_effect_into_its_slot() {
    let mut tally = Tally::default();
    for effect in [
        Effect::Created,
        Effect::Revised,
        Effect::Merged,
        Effect::Unchanged,
        Effect::Disputed,
        Effect::Tombstoned,
    ] {
        tally.record(&CandidateResult::Stored(Persisted { fact: None, effect }));
    }
    tally.record(&CandidateResult::Rejected {
        by: "schema".to_owned(),
        reason: "missing owner".to_owned(),
    });

    assert_eq!(
        tally,
        Tally {
            written: 3,
            unchanged: 1,
            disputed: 1,
            rejected: 2,
        }
    );
}

#[test]
fn an_event_whose_every_candidate_was_rejected_is_rejected() {
    let rejected = Tally {
        rejected: 2,
        ..Tally::default()
    };
    assert_eq!(rejected.outcome(), EventOutcome::Rejected);
    let mixed = Tally {
        rejected: 2,
        disputed: 1,
        ..Tally::default()
    };
    assert_eq!(mixed.outcome(), EventOutcome::Processed);
    assert_eq!(Tally::default().outcome(), EventOutcome::Processed);

    let summary = EventSummary::default();
    assert_eq!(summary.outcome, EventOutcome::Processed);
    assert_eq!(summary.staled, 0);
    assert_eq!(
        summary.with(EventOutcome::Rejected).outcome,
        EventOutcome::Rejected
    );
}

#[test]
fn a_summary_records_each_landed_fact_once_and_no_rejection() {
    let fact = BrainFactId::new("m_landed");
    let mut summary = EventSummary::default();
    for effect in [Effect::Created, Effect::Unchanged] {
        summary.record(&CandidateResult::Stored(Persisted {
            fact: Some(fact.clone()),
            effect,
        }));
    }
    summary.record(&CandidateResult::Stored(Persisted {
        fact: None,
        effect: Effect::Tombstoned,
    }));
    summary.record(&CandidateResult::Rejected {
        by: "schema".to_owned(),
        reason: "missing owner".to_owned(),
    });

    assert_eq!(summary.facts, vec![fact]);
    assert_eq!(summary.tally.written, 1);
    assert_eq!(summary.tally.unchanged, 1);
    assert_eq!(summary.tally.rejected, 2);
}
