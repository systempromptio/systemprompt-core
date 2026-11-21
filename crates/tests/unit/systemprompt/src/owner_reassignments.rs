use std::collections::BTreeSet;

use systemprompt_extension::{AdmittedOwnerReassignments, created_table_names};
use systemprompt_test_fixtures::fixture_admitted;

#[test]
fn each_owning_member_declares_reassignments_over_its_own_tables_only() {
    let admitted = fixture_admitted();
    let reassignments = AdmittedOwnerReassignments::of(admitted.built())
        .expect("every core member reassigns only its own tables");
    let owners: BTreeSet<&str> = reassignments
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
    for owned in reassignments.entries() {
        let member = admitted
            .built()
            .members()
            .find(|member| member.owner == owned.owner)
            .expect("an entry's owner is an admitted member");
        let own: BTreeSet<String> = member
            .sink
            .schemas()
            .iter()
            .flat_map(|schema| created_table_names(&schema.sql).expect("schema parses"))
            .collect();
        assert!(
            own.contains(owned.reassignment.table),
            "{} declares {} outside its own tables",
            owned.owner,
            owned.reassignment.table
        );
    }
}
