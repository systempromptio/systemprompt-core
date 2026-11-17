//! The test database is installed from
//! `systemprompt_test_migrate::core_members()`, an explicit list kept beside
//! the fixtures; the production installer reads the schema-bearing members of
//! the reference composition. The two lists must name the same members in the
//! same order, or the test database drifts from what an installation creates.

use systemprompt::database::schema_members;
use systemprompt::reference;

use crate::probe::build;

fn reference_schema_ids() -> Vec<&'static str> {
    let admitted = build(reference()).expect("the reference composition admits");
    schema_members(admitted.built())
        .iter()
        .map(|member| member.id())
        .collect()
}

fn fixture_ids(members: &[Box<dyn systemprompt::extension::Extension>]) -> Vec<&'static str> {
    members.iter().map(|member| member.metadata().id).collect()
}

#[test]
fn the_fixture_schema_members_are_the_reference_schema_members_in_order() {
    assert_eq!(
        fixture_ids(&systemprompt_test_fixtures::core_members()),
        reference_schema_ids()
    );
}

#[test]
fn a_fixture_list_missing_one_member_does_not_match_the_reference() {
    let mut members = systemprompt_test_fixtures::core_members();
    let dropped = members.remove(members.len() / 2);
    assert_ne!(
        fixture_ids(&members),
        reference_schema_ids(),
        "dropping {} must be detected",
        dropped.metadata().id
    );
}
