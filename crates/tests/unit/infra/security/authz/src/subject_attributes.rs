use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_security::authz::subject::{
    ROLE_PRECEDENCE, SharedSubjectAttributeProvider, SubjectAttributeProvider, SubjectAttributes,
    SubjectDimension, dimensions_of, gather_subject_attributes,
};
use systemprompt_security::authz::types::RuleType;

#[derive(Debug)]
struct StaticProvider {
    dimension: SubjectDimension,
    values: Vec<String>,
}

#[async_trait]
impl SubjectAttributeProvider for StaticProvider {
    fn dimension(&self) -> SubjectDimension {
        self.dimension.clone()
    }

    async fn values_for(&self, _user_id: &UserId) -> Vec<String> {
        self.values.clone()
    }
}

fn provider(
    slug: &'static str,
    precedence: u16,
    values: &[&str],
) -> SharedSubjectAttributeProvider {
    std::sync::Arc::new(StaticProvider {
        dimension: SubjectDimension {
            rule_type: RuleType::extension(slug).expect("slug must be a valid extension dimension"),
            label: slug,
            precedence,
        },
        values: values.iter().map(|v| (*v).to_owned()).collect(),
    })
}

#[test]
fn an_empty_attribute_set_reports_no_values_for_any_dimension() {
    let attributes = SubjectAttributes::new();

    assert!(attributes.is_empty());
    assert!(attributes.values(&RuleType::ROLE).is_empty());
    assert!(
        attributes
            .values(&RuleType::extension("department").unwrap())
            .is_empty()
    );
}

#[test]
fn inserting_a_dimension_makes_only_that_dimension_readable() {
    let mut attributes = SubjectAttributes::new();
    attributes.insert(
        RuleType::extension("department").unwrap(),
        vec!["engineering".to_owned()],
    );

    assert!(!attributes.is_empty());
    assert_eq!(
        attributes.values(&RuleType::extension("department").unwrap()),
        ["engineering".to_owned()]
    );
    assert!(
        attributes
            .values(&RuleType::extension("clearance").unwrap())
            .is_empty(),
        "an unregistered dimension must read as absent, not panic"
    );
}

#[test]
fn a_later_insert_replaces_the_values_of_the_same_dimension() {
    let dimension = RuleType::extension("cost_centre").unwrap();
    let mut attributes = SubjectAttributes::new();
    attributes.insert(
        dimension.clone(),
        vec!["cc-1".to_owned(), "cc-2".to_owned()],
    );
    attributes.insert(dimension.clone(), vec!["cc-9".to_owned()]);

    assert_eq!(attributes.values(&dimension), ["cc-9".to_owned()]);
}

#[test]
fn collecting_from_an_iterator_yields_the_same_set_as_repeated_inserts() {
    let dept = RuleType::extension("department").unwrap();
    let clearance = RuleType::extension("clearance").unwrap();

    let collected: SubjectAttributes = [
        (dept.clone(), vec!["ops".to_owned()]),
        (clearance.clone(), vec!["secret".to_owned()]),
    ]
    .into_iter()
    .collect();

    let mut inserted = SubjectAttributes::new();
    inserted.insert(dept, vec!["ops".to_owned()]);
    inserted.insert(clearance, vec!["secret".to_owned()]);

    assert_eq!(collected, inserted);
}

#[test]
fn dimensions_are_derived_from_the_providers_in_registration_order() {
    let providers = vec![
        provider("department", 100, &["engineering"]),
        provider("clearance", ROLE_PRECEDENCE + 10, &["secret"]),
    ];

    let dimensions = dimensions_of(&providers);

    assert_eq!(
        dimensions
            .iter()
            .map(|d| d.rule_type.as_str())
            .collect::<Vec<_>>(),
        ["department", "clearance"]
    );
    assert_eq!(dimensions[0].precedence, 100);
    assert_eq!(dimensions[1].precedence, ROLE_PRECEDENCE + 10);
}

#[test]
fn dimensions_of_no_providers_is_empty() {
    assert!(dimensions_of(&[]).is_empty());
}

#[tokio::test]
async fn gathering_snapshots_every_providers_values_under_its_own_dimension() {
    let providers = vec![
        provider("department", 100, &["engineering", "platform"]),
        provider("clearance", 150, &[]),
    ];
    let user = UserId::new("user-gather");

    let attributes = gather_subject_attributes(&providers, &user).await;

    assert_eq!(
        attributes.values(&RuleType::extension("department").unwrap()),
        ["engineering".to_owned(), "platform".to_owned()]
    );
    assert!(
        attributes
            .values(&RuleType::extension("clearance").unwrap())
            .is_empty(),
        "a provider returning nothing must leave its dimension unmatchable"
    );
    assert!(
        !attributes.is_empty(),
        "a dimension present with zero values is still a recorded dimension"
    );
}

#[test]
fn an_extension_dimension_may_not_shadow_a_core_dimension_or_be_malformed() {
    for rejected in ["user", "role", "", "_dept", "dept_", "Dept", "cost centre"] {
        assert!(
            RuleType::extension(rejected).is_err(),
            "{rejected:?} must not be mintable as an extension dimension"
        );
    }
    assert_eq!(
        RuleType::extension("cost_centre_2").unwrap().as_str(),
        "cost_centre_2"
    );
}
