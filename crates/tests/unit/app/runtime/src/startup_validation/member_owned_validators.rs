//! The AI, agent, MCP and content startup validators are contributed by
//! their owning members, so a composition without the member carries none of
//! its requirements, and one with it still enforces them.

use systemprompt_extension::admitted::{AdmittedMember, AdmittedParts};
use systemprompt_extension::contribute::{ContributionSink, DomainConfigFn};
use systemprompt_extension::{Admitted, Contributions, ExtensionMetadata, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::validators::{
    AiConfigValidator, ContentConfigValidator, StartupValidationReport,
};
use systemprompt_runtime::StartupValidator;

use crate::boot::{BootOptions, boot};

pub(super) fn admitted_with(id: &'static str, validator: Option<DomainConfigFn>) -> Admitted {
    let owner = ExtensionId::new(id);
    let mut sink = ContributionSink::default();
    if let Some(validator) = validator {
        Contributions::__new(owner.clone(), &mut sink).domain_config(validator);
    }
    Admitted::__from_facade(AdmittedParts {
        withheld: Vec::new(),
        members: vec![AdmittedMember {
            secret_grants: Vec::new(),
            owner,
            metadata: ExtensionMetadata {
                id,
                name: id,
                version: "0.0.1",
            },
            standing: StandingTag::Install,
            sink,
        }],
        ..AdmittedParts::default()
    })
}

fn validate_without_content_config(admitted: &Admitted) -> StartupValidationReport {
    let fixture = boot(&BootOptions::default());
    let mut config = fixture
        .boot
        .try_init_config(None)
        .expect("init config from profile")
        .as_ref()
        .clone();
    config.content_config_path = std::path::Path::new(&config.services_path)
        .join("content/absent.yaml")
        .display()
        .to_string();
    let mut validator = StartupValidator::new(fixture.boot.clone()).expect("startup validator");
    validator.validate(&config, admitted.built())
}

#[test]
fn without_the_ai_member_no_ai_domain_is_validated() {
    let report = validate_without_content_config(&admitted_with("site", None));
    assert!(
        !report.domains.iter().any(|d| d.domain == "ai"),
        "a composition without the ai member must not run the AI validator"
    );
}

#[test]
fn with_the_ai_member_the_ai_domain_is_validated() {
    let report =
        validate_without_content_config(&admitted_with("ai", Some(AiConfigValidator::boxed)));
    assert!(
        report.domains.iter().any(|d| d.domain == "ai"),
        "the ai member's validator must run when the member is admitted"
    );
}

#[test]
fn without_the_content_member_a_missing_content_config_is_not_an_error() {
    let report = validate_without_content_config(&admitted_with("site", None));
    assert!(
        !report
            .domains
            .iter()
            .any(|d| d.domain == "content" && d.has_errors()),
        "a composition without the content member must not require its config"
    );
}

#[test]
fn with_the_content_member_a_missing_content_config_is_an_error() {
    let report = validate_without_content_config(&admitted_with(
        "content",
        Some(ContentConfigValidator::boxed),
    ));
    let content = report
        .domains
        .iter()
        .find(|d| d.domain == "content")
        .expect("the content member's validator must run");
    assert!(
        content
            .errors
            .iter()
            .any(|e| e.field == "paths.content_config"),
        "a missing content config must still be refused, got: {:?}",
        content.errors
    );
}
