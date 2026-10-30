use systemprompt_brain::BrainError;
use systemprompt_brain::service::authority::{
    BrainCaller, may_decide_proposals, may_enter, may_govern, may_view,
};
use systemprompt_identifiers::WorkflowId;
use systemprompt_models::brain::{FactScope, WorkflowProfileDecl};

use crate::fixtures::kind;
use crate::recall_fixtures::uid;

fn caller(user: &str, roles: &[&str]) -> BrainCaller {
    BrainCaller::new(uid(user), roles.iter().map(|r| (*r).to_owned()).collect())
}

fn workflow(kind_id: &str, role: Option<&str>) -> WorkflowProfileDecl {
    WorkflowProfileDecl {
        id: WorkflowId::try_new("delivery").expect("workflow"),
        name: "Delivery".to_owned(),
        kinds: vec![kind(kind_id)],
        confirm_role: role.map(str::to_owned),
        quotas: Default::default(),
        budget_tokens: None,
    }
}

fn refused(result: Result<(), BrainError>) -> bool {
    matches!(result, Err(BrainError::Forbidden { .. }))
}

#[test]
fn a_personal_fact_is_governed_by_its_owner_alone() {
    let scope = FactScope::User(uid("bob"));
    let flows = [workflow("risk", Some("lead"))];
    assert!(may_govern(&caller("bob", &[]), &scope, &kind("risk"), &flows).is_ok());
    assert!(refused(may_govern(
        &caller("carol", &[]),
        &scope,
        &kind("risk"),
        &flows
    )));
    assert!(refused(may_govern(
        &caller("admin", &["admin"]),
        &scope,
        &kind("risk"),
        &flows
    )));
    assert!(refused(may_govern(
        &caller("lead", &["lead"]),
        &scope,
        &kind("risk"),
        &flows
    )));
}

#[test]
fn a_shared_fact_needs_an_admin_or_the_named_role() {
    let flows = [workflow("risk", Some("lead"))];
    for scope in [
        FactScope::Org,
        FactScope::Project("apollo".to_owned()),
        FactScope::Group("platform".to_owned()),
    ] {
        assert!(may_govern(&caller("ann", &["admin"]), &scope, &kind("risk"), &flows).is_ok());
        assert!(
            may_govern(
                &caller("lee", &["user", "lead"]),
                &scope,
                &kind("risk"),
                &flows
            )
            .is_ok()
        );
        assert!(refused(may_govern(
            &caller("sam", &["user"]),
            &scope,
            &kind("risk"),
            &flows
        )));
    }
}

#[test]
fn a_role_named_for_another_kind_or_no_role_at_all_is_refused() {
    let org = FactScope::Org;
    let lead = caller("lee", &["lead"]);
    assert!(refused(may_govern(
        &lead,
        &org,
        &kind("other"),
        &[workflow("risk", Some("lead"))]
    )));
    assert!(refused(may_govern(
        &lead,
        &org,
        &kind("risk"),
        &[workflow("risk", None)]
    )));
    assert!(refused(may_govern(&lead, &org, &kind("risk"), &[])));
}

#[test]
fn viewing_entering_and_deciding_proposals() {
    let bobs = FactScope::User(uid("bob"));
    assert!(may_view(&caller("bob", &[]), &bobs).is_ok());
    assert!(refused(may_view(&caller("carol", &["admin"]), &bobs)));
    assert!(may_view(&caller("carol", &[]), &FactScope::Org).is_ok());
    assert!(may_enter(&uid("bob"), &bobs).is_ok());
    assert!(refused(may_enter(&uid("carol"), &bobs)));
    assert!(may_decide_proposals(&caller("ann", &["admin"])).is_ok());
    assert!(refused(may_decide_proposals(&caller("sam", &["user"]))));
}
