use std::collections::BTreeSet;
use std::sync::Arc;

use systemprompt_database::{Database, DbPool};
use systemprompt_extension::AdmittedOwnerReassignments;
use systemprompt_extension::prelude::Dependencies;
use systemprompt_test_fixtures::{fixture_admitted, fixture_kernel, lazy_pg_pool};

fn admitted_reassignments() -> AdmittedOwnerReassignments {
    let pool: DbPool = Arc::new(Database::from_pools(lazy_pg_pool(), None));
    let kernel = fixture_kernel(&pool, Dependencies::new().with(Arc::clone(&pool)));
    AdmittedOwnerReassignments::of(fixture_admitted().built(), &kernel.resolved())
        .expect("every core member reassigns only its own tables")
}

#[test]
fn each_owning_member_declares_its_own_reassignment() {
    let admitted = admitted_reassignments();
    let owners: BTreeSet<&str> = admitted
        .entries()
        .iter()
        .map(|owned| owned.owner.as_str())
        .collect();
    assert_eq!(
        owners,
        BTreeSet::from([
            "agent",
            "ai",
            "analytics",
            "content",
            "events",
            "files",
            "logging",
            "mcp"
        ])
    );
    for owned in admitted.entries() {
        assert_eq!(
            owned.reassignment.domain(),
            owned.owner.as_str(),
            "a member's reassignment is its own"
        );
        assert!(!owned.reassignment.columns().is_empty());
    }
}
