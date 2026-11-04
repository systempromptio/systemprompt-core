//! `admit::admitted_by_sql` restates every predicate of the tier SQL over
//! a loaded fact: scope level, status (confirmed; proposed only for its
//! owner; stale only with the marker policy), review, validity and
//! `observed_at <= as_of`, each withheld under the reason the SQL path
//! implies.

use chrono::{DateTime, Duration, Utc};
use systemprompt_brain::recall::admit::{Admission, admitted_by_sql};
use systemprompt_identifiers::UserId;
use systemprompt_models::brain::{BrainFact, FactScope, FactStatus, ScopeLevel, WithheldReason};

use crate::fixtures::at;
use crate::recall_fixtures::{brain_fact, uid, with_scope};

const READER: &str = "reader";

fn as_of() -> DateTime<Utc> {
    at(1_700_000_060)
}

struct Terms {
    user: UserId,
    scopes: Vec<ScopeLevel>,
}

impl Terms {
    fn reading(scopes: &[ScopeLevel]) -> Self {
        Self {
            user: uid(READER),
            scopes: scopes.to_vec(),
        }
    }

    fn admission(&self, include_stale: bool) -> Admission<'_> {
        Admission {
            user: &self.user,
            scopes: &self.scopes,
            include_stale,
            as_of: as_of(),
        }
    }
}

fn check(fact: &BrainFact, include_stale: bool) -> Option<WithheldReason> {
    let terms = Terms::reading(&ScopeLevel::ALL);
    admitted_by_sql(fact, &terms.admission(include_stale))
}

#[test]
fn a_confirmed_fact_in_scope_observed_by_as_of_is_admitted() {
    assert_eq!(check(&brain_fact(1, "a"), false), None);
}

#[test]
fn a_proposed_fact_is_admitted_only_for_its_owner() {
    let mut own = brain_fact(2, "a");
    own.status = FactStatus::Proposed;
    own.owner_user_id = Some(uid(READER));
    assert_eq!(check(&own, false), None);

    let mut strangers = own.clone();
    strangers.owner_user_id = Some(uid("stranger"));
    assert_eq!(check(&strangers, false), Some(WithheldReason::Scope));
    let mut unowned = own;
    unowned.owner_user_id = None;
    assert_eq!(check(&unowned, false), Some(WithheldReason::Scope));
}

#[test]
fn every_status_the_sql_refuses_is_withheld_under_its_reason() {
    let mut stale = brain_fact(3, "a");
    stale.status = FactStatus::Stale;
    assert_eq!(check(&stale, false), Some(WithheldReason::Stale));
    assert_eq!(check(&stale, true), None);

    let mut disputed = brain_fact(4, "a");
    disputed.status = FactStatus::Disputed;
    assert_eq!(check(&disputed, true), Some(WithheldReason::Disputed));
    for status in [
        FactStatus::Candidate,
        FactStatus::Superseded,
        FactStatus::Retired,
    ] {
        let mut fact = brain_fact(5, "a");
        fact.status = status;
        assert_eq!(
            check(&fact, true),
            Some(WithheldReason::Scope),
            "{status:?}"
        );
    }
}

#[test]
fn a_scope_level_the_recall_does_not_read_is_withheld_as_scope() {
    let project = with_scope(brain_fact(6, "a"), FactScope::Project("proj-x".to_owned()));
    let terms = Terms::reading(&[ScopeLevel::User, ScopeLevel::Org]);
    assert_eq!(
        admitted_by_sql(&project, &terms.admission(false)),
        Some(WithheldReason::Scope)
    );
    assert_eq!(
        admitted_by_sql(&brain_fact(7, "a"), &terms.admission(false)),
        None
    );
}

#[test]
fn review_validity_and_observation_are_judged_as_of_the_recall() {
    let as_of = as_of();

    let mut due = brain_fact(8, "a");
    due.review_after = Some(as_of - Duration::seconds(1));
    assert_eq!(check(&due, false), Some(WithheldReason::ReviewDue));
    due.review_after = Some(as_of);
    assert_eq!(check(&due, false), None);

    let mut not_yet = brain_fact(9, "a");
    not_yet.valid_from = Some(as_of + Duration::seconds(1));
    assert_eq!(check(&not_yet, false), Some(WithheldReason::Scope));
    let mut ended = brain_fact(10, "a");
    ended.valid_to = Some(as_of);
    assert_eq!(check(&ended, false), Some(WithheldReason::Scope));
    ended.valid_to = Some(as_of + Duration::seconds(1));
    assert_eq!(check(&ended, false), None);

    let mut later = brain_fact(11, "a");
    later.observed_at = as_of + Duration::seconds(1);
    assert_eq!(check(&later, false), Some(WithheldReason::Scope));
    later.observed_at = as_of;
    assert_eq!(check(&later, false), None);
}
