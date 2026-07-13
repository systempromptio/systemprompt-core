//! Drives the error-accumulation arm of `FilesConfigValidator::validate`:
//! the profile's storage root sits below a regular file, so
//! `ensure_storage_structure` cannot create it and every failure is folded
//! into the report with the permissions suggestion.

use systemprompt_files::FilesConfig;
use systemprompt_models::AppPaths;
use systemprompt_runtime::FilesConfigValidator;
use systemprompt_traits::DomainConfig;

use crate::boot::{BootOptions, boot};

#[test]
fn broken_storage_root_reports_storage_errors_with_suggestion() {
    let Some(_fixture) = boot(&BootOptions {
        broken_storage: true,
        ..BootOptions::default()
    }) else {
        return;
    };
    let profile = systemprompt_config::ProfileBootstrap::get().expect("profile installed");
    let app_paths = AppPaths::from_profile(&profile.paths).expect("app paths");
    FilesConfig::init(&app_paths).expect("init files config");

    let validator = FilesConfigValidator::new();
    let report = validator.validate().expect("validate returns a report");

    assert_eq!(report.domain, "files");
    assert!(
        !report.errors.is_empty(),
        "uncreatable storage root must produce errors"
    );
    for error in &report.errors {
        assert_eq!(error.field, "storage");
        assert!(
            error.message.contains("Failed to create storage root"),
            "got: {}",
            error.message
        );
        assert_eq!(
            error.suggestion.as_deref(),
            Some("Check filesystem permissions for storage directory")
        );
    }
}
