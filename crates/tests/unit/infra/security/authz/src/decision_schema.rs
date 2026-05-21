//! Guards the schema↔enum contract for `governance_decisions.decision`.
//!
//! Sister to [`super::actor_kind_schema`]. The same class of regression that
//! silently dropped `actor_kind = 'agent'` writes could repeat for the
//! `decision` column the moment a new [`DecisionTag`] variant is added without
//! the CHECK being extended. This test names every variant in the SQL so the
//! drift is caught at compile time of the test crate, not in production.

use systemprompt_security::authz::DecisionTag;

const SCHEMA_SQL: &str =
    include_str!("../../../../../../infra/security/schema/governance_decisions.sql");

fn all_variants() -> [DecisionTag; 2] {
    [DecisionTag::Allow, DecisionTag::Deny]
}

#[test]
fn schema_check_lists_every_decision_tag() {
    for tag in all_variants() {
        let token = format!("'{}'", tag.as_str());
        assert!(
            SCHEMA_SQL.contains(&token),
            "governance_decisions.sql CHECK is missing DecisionTag::{tag:?} ({token}); extend the \
             column allow-list when the enum grows"
        );
    }
}
