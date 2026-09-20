use systemprompt_models::Config;
use systemprompt_runtime::StartupValidator;

use crate::boot::{BootOptions, boot};

#[test]
fn domain_validation_failure_is_aggregated_and_gates_extension_validation() {
    let fixture = boot(&BootOptions::default()).expect("runtime database/profile fixture");
    systemprompt_config::try_init_config(None).expect("initialize config from fixture profile");
    let mut config = Config::get().expect("installed config").clone();
    let blocker = fixture._tmp.path().join("skills-is-a-file");
    std::fs::write(&blocker, "not a directory").expect("skills path blocker");
    config.skills_path = blocker.display().to_string();

    let report = StartupValidator::new().validate(&config);

    let skills = report
        .domains
        .iter()
        .find(|domain| domain.domain == "skills")
        .expect("skills validation result retained");
    assert_eq!(skills.errors.len(), 1);
    assert_eq!(skills.errors[0].field, "skills_validation");
    assert!(
        skills.errors[0]
            .message
            .contains("Validation error: Failed to load config: Cannot read skills directory"),
        "unexpected aggregation message: {}",
        skills.errors[0].message
    );
    assert!(
        report.domains.iter().any(|domain| domain.domain == "files"),
        "a failing domain must not discard earlier successful reports"
    );
    assert!(
        report.extensions.is_empty(),
        "domain validation errors must gate extension validation"
    );
}
