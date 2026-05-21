//! Guards the schema↔enum contract for `governance_decisions.decision`.
//!
//! Sister to [`super::actor_kind_schema`]: asserts every [`DecisionTag`]
//! variant appears in the column's CHECK allow-list so extending the enum
//! without extending the constraint fails this test.

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
