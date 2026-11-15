//! T8: subject dimensions are owned once. An install declaring a reserved
//! core dimension is refused with `ReservedDimension`, two members declaring
//! one dimension with `DimensionConflict`, and the set is ordered core first,
//! so the core owner is always the `first` a conflict names. The kernel builds
//! the set at resolve, so the composition-level refusals are
//! `CompositionError` variants.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt::{CompositionError, reference};
use systemprompt_extension::{Admitted, Resolved, StandingTag};
use systemprompt_governance::authz::{
    RuleType, SharedSubjectAttributeProvider, SubjectAttributeError, SubjectAttributeProvider,
    SubjectDimension, SubjectProviderEntry, SubjectProviderError, SubjectProviderSet,
};
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_runtime::Kernel;

use crate::probe::{Probe, build, build_with, kernel};

#[derive(Debug)]
struct Dimension(RuleType);

#[async_trait]
impl SubjectAttributeProvider for Dimension {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: self.0.clone(),
            label: "Probe",
            precedence: 500,
        }
    }

    async fn values_for(&self, _user_id: &UserId) -> Result<Vec<String>, SubjectAttributeError> {
        Ok(Vec::new())
    }
}

fn provider(rule_type: RuleType) -> SharedSubjectAttributeProvider {
    Arc::new(Dimension(rule_type))
}

fn entry(owner: &str, standing: StandingTag, rule_type: RuleType) -> SubjectProviderEntry {
    SubjectProviderEntry {
        owner: ExtensionId::new(owner),
        standing,
        provider: provider(rule_type),
    }
}

#[test]
fn an_install_declaring_a_reserved_core_dimension_is_refused() {
    for reserved in [
        RuleType::USER,
        RuleType::ROLE,
        RuleType::extension_static("traffic"),
    ] {
        let err = SubjectProviderSet::new(vec![entry("acme", StandingTag::Install, reserved)])
            .expect_err("a reserved dimension is refused for an install");
        assert!(
            matches!(&err, SubjectProviderError::ReservedDimension { owner, .. } if owner.as_str() == "acme"),
            "unexpected error: {err:?}"
        );
    }
}

#[test]
fn a_core_member_may_declare_its_own_reserved_dimension_but_not_user_or_role() {
    assert!(
        SubjectProviderSet::new(vec![entry(
            "authz",
            StandingTag::Core,
            RuleType::extension_static("traffic"),
        )])
        .is_ok()
    );
    for request_dimension in [RuleType::USER, RuleType::ROLE] {
        assert!(matches!(
            SubjectProviderSet::new(vec![entry("authz", StandingTag::Core, request_dimension)]),
            Err(SubjectProviderError::ReservedDimension { .. })
        ));
    }
}

#[test]
fn two_installs_declaring_one_dimension_conflict() {
    let department = RuleType::extension_static("department");
    let err = SubjectProviderSet::new(vec![
        entry("first", StandingTag::Install, department.clone()),
        entry("second", StandingTag::Install, department),
    ])
    .expect_err("a duplicate dimension is refused");
    assert!(
        matches!(&err, SubjectProviderError::DimensionConflict { first, second, .. }
            if first.as_str() == "first" && second.as_str() == "second"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn a_conflict_names_the_core_owner_first_whatever_the_order() {
    let department = RuleType::extension_static("department");
    let err = SubjectProviderSet::new(vec![
        entry("install", StandingTag::Install, department.clone()),
        entry("core", StandingTag::Core, department),
    ])
    .expect_err("a duplicate dimension is refused");
    assert!(
        matches!(&err, SubjectProviderError::DimensionConflict { first, second, .. }
            if first.as_str() == "core" && second.as_str() == "install"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn distinct_dimensions_build_core_first() {
    let set = SubjectProviderSet::new(vec![
        entry(
            "install",
            StandingTag::Install,
            RuleType::extension_static("site"),
        ),
        entry(
            "core",
            StandingTag::Core,
            RuleType::extension_static("traffic"),
        ),
    ])
    .expect("distinct dimensions build");
    let order: Vec<String> = set
        .registered_dimensions()
        .iter()
        .map(|d| d.as_str().to_owned())
        .collect();
    assert_eq!(order, ["traffic", "site"]);
}

fn resolve(admitted: &Admitted) -> Result<Kernel, CompositionError> {
    let mut kernel = kernel();
    kernel.resolve(admitted).map_err(CompositionError::from)?;
    Ok(kernel)
}

fn dimension_probe(id: &'static str, rule_type: &'static str) -> Probe {
    Probe::new(id, move |c| {
        c.subject_attributes(move |_: &Resolved<'_>| {
            Ok(provider(RuleType::extension_static(rule_type)))
        });
    })
}

#[test]
fn an_install_member_declaring_traffic_is_refused_when_the_kernel_resolves() {
    let admitted = build_with(dimension_probe("probe-traffic", "traffic"))
        .expect("the composition admits the probe");
    let err = resolve(&admitted).expect_err("the install's reserved dimension is refused");
    assert!(
        matches!(&err, CompositionError::ReservedDimension { owner, .. } if owner.as_str() == "probe-traffic"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn two_install_members_declaring_one_dimension_conflict_when_the_kernel_resolves() {
    let admitted = build(
        reference()
            .with(dimension_probe("probe-first", "department"))
            .with(dimension_probe("probe-second", "department")),
    )
    .expect("the composition admits both probes");
    let err = resolve(&admitted).expect_err("a duplicate dimension is refused");
    assert!(
        matches!(&err, CompositionError::DimensionConflict { first, second, .. }
            if first.as_str() == "probe-first" && second.as_str() == "probe-second"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn the_reference_composition_resolves_core_traffic_alone_as_the_negative_control() {
    let admitted = build(reference()).expect("the reference composition admits");
    let kernel = resolve(&admitted).expect("core's providers build");
    let set = kernel
        .state()
        .get::<Arc<SubjectProviderSet>>()
        .expect("the kernel inserts the subject provider set");
    assert!(set.find("traffic").is_some());
}
