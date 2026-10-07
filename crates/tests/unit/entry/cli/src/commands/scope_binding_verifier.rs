//! `SubjectProviderSet::verify_scope_bindings` — the owner-membership check
//! shared by the admin HTTP route and `admin users api-key issue`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;
use systemprompt_identifiers::{ScopeDimension, UserId};
use systemprompt_models::attribution::ScopeBinding;
use systemprompt_security::authz::{
    AuthzError, RuleType, ScopeBindingError, SubjectAttributeProvider, SubjectDimension,
    SubjectProviderSet,
};

const PROJECT: RuleType = RuleType::extension_static("project");

#[derive(Debug)]
struct ProjectProvider;

#[async_trait::async_trait]
impl SubjectAttributeProvider for ProjectProvider {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: PROJECT,
            label: "Project",
            precedence: 300,
        }
    }

    async fn values_for(&self, user_id: &UserId) -> Result<Vec<String>, AuthzError> {
        if user_id.as_str() == "member" {
            return Ok(vec!["p-primary".to_owned(), "p-other".to_owned()]);
        }
        Ok(Vec::new())
    }
}

fn providers() -> SubjectProviderSet {
    SubjectProviderSet::from_providers(vec![Arc::new(ProjectProvider)])
}

fn binding(dimension: &str, value: &str) -> ScopeBinding {
    ScopeBinding {
        dimension: ScopeDimension::new(dimension),
        value: value.to_owned(),
    }
}

#[tokio::test]
async fn a_value_the_owner_holds_is_accepted() {
    providers()
        .verify_scope_bindings(&UserId::new("member"), &[binding("project", "p-other")])
        .await
        .expect("the owner holds p-other");
}

#[tokio::test]
async fn no_bindings_need_no_provider() {
    SubjectProviderSet::default()
        .verify_scope_bindings(&UserId::new("anyone"), &[])
        .await
        .expect("nothing to verify");
}

#[tokio::test]
async fn a_value_the_owner_does_not_hold_is_refused() {
    let err = providers()
        .verify_scope_bindings(&UserId::new("member"), &[binding("project", "p-foreign")])
        .await
        .expect_err("p-foreign is not the owner's");
    assert!(
        matches!(&err, ScopeBindingError::NotAMember { value, .. } if value == "p-foreign"),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_non_member_is_refused_every_value() {
    let err = providers()
        .verify_scope_bindings(&UserId::new("outsider"), &[binding("project", "p-primary")])
        .await
        .expect_err("an outsider holds no project");
    assert!(
        matches!(err, ScopeBindingError::NotAMember { .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_unregistered_dimension_is_refused() {
    let err = providers()
        .verify_scope_bindings(
            &UserId::new("member"),
            &[
                binding("project", "p-primary"),
                binding("cost_centre", "cc-1"),
            ],
        )
        .await
        .expect_err("no provider registers cost_centre");
    assert!(
        matches!(&err, ScopeBindingError::UnknownDimension(d) if d.as_str() == "cost_centre"),
        "{err:?}"
    );
}
