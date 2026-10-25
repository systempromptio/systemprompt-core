//! An ownership scope that claims a kind with no ids is still a scope.

use systemprompt_governance::authz::{EntityKind, IngestScope};

#[test]
fn a_kind_added_with_no_ids_still_makes_the_scope_a_scope() {
    let scope = IngestScope::new().with_kind(EntityKind::Marketplace, Vec::<String>::new());

    assert!(
        !scope.is_unscoped(),
        "a kind claimed with an empty id list is an assertion of ownership over nothing, not an \
         absence of ownership"
    );
    assert!(
        !scope.owns(EntityKind::Marketplace, "anything-at-all"),
        "a scope that owns no ids owns no id"
    );
}
