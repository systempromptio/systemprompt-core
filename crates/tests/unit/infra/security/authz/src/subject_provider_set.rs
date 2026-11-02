use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_governance::authz::{AuthzError, default_attribution_value};
use systemprompt_governance::authz::subject::{
    SharedSubjectAttributeProvider, SubjectAttributeProvider, SubjectAttributeSource,
    SubjectDimension, SubjectProviderSet, SubjectSourceError,
};
use systemprompt_governance::authz::types::RuleType;
use systemprompt_identifiers::UserId;

#[derive(Debug)]
struct Fixed {
    slug: &'static str,
    values: Option<Vec<String>>,
    known: Option<Vec<String>>,
    known_fails: bool,
}

#[async_trait]
impl SubjectAttributeProvider for Fixed {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: RuleType::extension(self.slug).expect("valid slug"),
            label: self.slug,
            precedence: 100,
        }
    }

    async fn values_for(&self, _user_id: &UserId) -> Result<Vec<String>, AuthzError> {
        self.values
            .clone()
            .ok_or_else(|| AuthzError::Validation(format!("{} lookup down", self.slug)))
    }

    async fn known_values(&self) -> Result<Option<Vec<String>>, AuthzError> {
        if self.known_fails {
            return Err(AuthzError::Validation("catalogue down".to_owned()));
        }
        Ok(self.known.clone())
    }
}

fn ok(slug: &'static str, values: &[&str]) -> SharedSubjectAttributeProvider {
    Arc::new(Fixed {
        slug,
        values: Some(values.iter().map(|v| (*v).to_owned()).collect()),
        known: Some(vec!["eng".to_owned(), "ops".to_owned()]),
        known_fails: false,
    })
}

fn broken(slug: &'static str) -> SharedSubjectAttributeProvider {
    Arc::new(Fixed {
        slug,
        values: None,
        known: None,
        known_fails: true,
    })
}

fn dim(slug: &'static str) -> SubjectDimension {
    SubjectDimension {
        rule_type: RuleType::extension(slug).expect("valid slug"),
        label: slug,
        precedence: 100,
    }
}

fn user() -> UserId {
    UserId::new("user-subject-set")
}

#[test]
fn find_and_registered_dimensions_reflect_providers() {
    let set = SubjectProviderSet::from_providers(vec![ok("department", &["eng"]), ok("site", &[])]);
    let department = RuleType::extension("department").expect("valid slug");
    assert!(set.find(department.as_str()).is_some());
    assert!(set.find("ext:absent").is_none());
    let registered = set.registered_dimensions();
    assert_eq!(registered.len(), 2);
    assert_eq!(registered[0], department);
    assert_eq!(set.iter().count(), 2);
    assert_eq!(SubjectAttributeSource::dimensions(&set).len(), 2);
}

#[test]
fn empty_set_has_no_dimensions() {
    let set = SubjectProviderSet::default();
    assert!(set.registered_dimensions().is_empty());
    assert!(SubjectAttributeSource::dimensions(&set).is_empty());
}

#[tokio::test]
async fn gather_collects_every_provider() {
    let set = SubjectProviderSet::from_providers(vec![ok("department", &["eng", "ops"])]);
    let attrs = set.gather(&user()).await.expect("gather");
    assert_eq!(attrs.values(&dim("department").rule_type), ["eng", "ops"]);
}

#[tokio::test]
async fn gather_propagates_provider_error() {
    let set = SubjectProviderSet::from_providers(vec![broken("department")]);
    let err = set.gather(&user()).await.expect_err("provider failure");
    assert!(matches!(err, AuthzError::Validation(_)));
}

#[tokio::test]
async fn values_for_known_dimension_returns_values() {
    let set = SubjectProviderSet::from_providers(vec![ok("department", &["eng"])]);
    let values = set
        .values_for(&user(), &dim("department"))
        .await
        .expect("values");
    assert_eq!(values, ["eng"]);
}

#[tokio::test]
async fn values_for_unknown_dimension_is_refused() {
    let set = SubjectProviderSet::from_providers(vec![ok("department", &["eng"])]);
    let err = set
        .values_for(&user(), &dim("site"))
        .await
        .expect_err("unknown");
    match err {
        SubjectSourceError::UnknownDimension(name) => {
            assert_eq!(name, dim("site").rule_type.as_str());
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn values_for_wraps_provider_failure() {
    let set = SubjectProviderSet::from_providers(vec![broken("department")]);
    let err = set
        .values_for(&user(), &dim("department"))
        .await
        .expect_err("failure");
    match err {
        SubjectSourceError::Provider { dimension, source } => {
            assert_eq!(dimension, dim("department").rule_type.as_str());
            assert!(source.to_string().contains("lookup down"));
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn resolve_all_returns_each_dimension() {
    let set =
        SubjectProviderSet::from_providers(vec![ok("department", &["eng"]), ok("site", &["lon"])]);
    let attrs = set.resolve_all(&user()).await.expect("resolve");
    assert_eq!(attrs.values(&dim("department").rule_type), ["eng"]);
    assert_eq!(attrs.values(&dim("site").rule_type), ["lon"]);
}

#[tokio::test]
async fn resolve_all_fails_closed_on_any_provider_error() {
    let set = SubjectProviderSet::from_providers(vec![ok("department", &["eng"]), broken("site")]);
    let err = set.resolve_all(&user()).await.expect_err("fails");
    assert!(
        matches!(err, SubjectSourceError::Provider { ref dimension, .. } if dimension == dim("site").rule_type.as_str())
    );
}

#[tokio::test]
async fn known_values_delegates_and_refuses_unknown() {
    let set = SubjectProviderSet::from_providers(vec![ok("department", &[]), broken("site")]);
    let known = set
        .known_values(&dim("department"))
        .await
        .expect("known")
        .expect("listed");
    assert_eq!(known, ["eng", "ops"]);
    assert!(matches!(
        set.known_values(&dim("site")).await,
        Err(SubjectSourceError::Provider { .. })
    ));
    assert!(matches!(
        set.known_values(&dim("region")).await,
        Err(SubjectSourceError::UnknownDimension(_))
    ));
}

#[derive(Debug)]
struct HeaderOnly;

#[async_trait]
impl SubjectAttributeProvider for HeaderOnly {
    fn dimension(&self) -> SubjectDimension {
        dim("tenant")
    }

    async fn values_for(&self, _user_id: &UserId) -> Result<Vec<String>, AuthzError> {
        Ok(vec!["acme".to_owned()])
    }

    fn default_attribution(&self) -> bool {
        false
    }
}

#[tokio::test]
async fn default_value_is_the_first_held_value_unless_header_only() {
    let header_only: SharedSubjectAttributeProvider = Arc::new(HeaderOnly);
    let set = SubjectProviderSet::from_providers(vec![
        ok("department", &["ops", "eng"]),
        ok("site", &[]),
        Arc::clone(&header_only),
    ]);
    let department = dim("department").rule_type;
    let site = dim("site").rule_type;
    let tenant = dim("tenant").rule_type;
    let value = |dimension: String| {
        let set = set.clone();
        async move { set.default_value(&user(), &dimension).await.expect("lookup") }
    };
    assert_eq!(value(department.as_str().to_owned()).await.as_deref(), Some("ops"));
    assert_eq!(value(site.as_str().to_owned()).await, None);
    assert_eq!(value(tenant.as_str().to_owned()).await, None, "header-only");
    assert_eq!(value("ext:absent".to_owned()).await, None, "unregistered");
    assert_eq!(
        default_attribution_value(&header_only, &["acme".to_owned()]),
        None
    );
}
