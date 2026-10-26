//! The subject source handle is unbound until the set exists: reads fail
//! closed with `NotReady`, then resolve through the bound source, and a second
//! bind is refused.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_models::authz::{
    RuleType, SubjectAttributeSource, SubjectAttributes, SubjectDimension, SubjectSourceError,
    SubjectSourceHandle,
};

const DEPARTMENT: RuleType = RuleType::extension_static("department");

struct Fixed(Vec<SubjectDimension>);

#[async_trait]
impl SubjectAttributeSource for Fixed {
    fn dimensions(&self) -> &[SubjectDimension] {
        &self.0
    }

    async fn values_for(
        &self,
        _user: &UserId,
        _dimension: &SubjectDimension,
    ) -> Result<Vec<String>, SubjectSourceError> {
        Ok(vec!["finance".to_owned()])
    }

    async fn resolve_all(&self, _user: &UserId) -> Result<SubjectAttributes, SubjectSourceError> {
        Ok(std::iter::once((DEPARTMENT, vec!["finance".to_owned()])).collect())
    }
}

fn dimension() -> SubjectDimension {
    SubjectDimension {
        rule_type: DEPARTMENT,
        label: "department",
        precedence: 100,
    }
}

#[tokio::test]
async fn an_unbound_handle_is_not_ready_and_a_bound_one_resolves() {
    let handle = SubjectSourceHandle::default();
    let user = UserId::new("user-1");
    assert!(handle.dimensions().is_empty());
    assert!(matches!(
        handle.resolve_all(&user).await,
        Err(SubjectSourceError::NotReady)
    ));
    assert!(matches!(
        handle.values_for(&user, &dimension()).await,
        Err(SubjectSourceError::NotReady)
    ));

    let clone = handle.clone();
    handle
        .bind(Arc::new(Fixed(vec![dimension()])))
        .expect("first bind");

    assert_eq!(clone.dimensions().len(), 1);
    assert_eq!(
        clone.values_for(&user, &dimension()).await.expect("values"),
        vec!["finance".to_owned()]
    );
    assert_eq!(
        clone
            .resolve_all(&user)
            .await
            .expect("all")
            .values(&DEPARTMENT),
        ["finance".to_owned()]
    );
    assert!(handle.bind(Arc::new(Fixed(Vec::new()))).is_err());
}
