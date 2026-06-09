#![allow(clippy::all)]

use systemprompt_config::SkillConfigValidator;
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

struct StubConfig {
    skills_path: Option<String>,
}

impl ConfigProvider for StubConfig {
    fn get(&self, key: &str) -> Option<String> {
        if key == "skills_path" {
            self.skills_path.clone()
        } else {
            None
        }
    }

    fn database_url(&self) -> &str {
        "postgres://u:p@localhost/db"
    }

    fn system_path(&self) -> &str {
        "/tmp"
    }

    fn api_port(&self) -> u16 {
        8080
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn config_with_path(path: &str) -> StubConfig {
    StubConfig {
        skills_path: Some(path.to_owned()),
    }
}

fn config_without_path() -> StubConfig {
    StubConfig { skills_path: None }
}

fn valid_skill_yaml() -> &'static str {
    "id: my-skill\nname: My Skill\ndescription: A test skill\n"
}

#[test]
fn skill_validator_domain_id() {
    let v = SkillConfigValidator::new();
    assert_eq!(v.domain_id(), "skills");
}

#[test]
fn skill_validator_priority() {
    let v = SkillConfigValidator::new();
    assert_eq!(v.priority(), 25);
}

#[test]
fn skill_validator_default_is_same_as_new() {
    let v1 = SkillConfigValidator::new();
    let v2 = SkillConfigValidator::default();
    assert_eq!(v1.domain_id(), v2.domain_id());
    assert_eq!(v1.priority(), v2.priority());
}

#[test]
fn load_succeeds_with_skills_path() {
    let tmp = tempfile::tempdir().unwrap();
    let mut v = SkillConfigValidator::new();
    let cfg = config_with_path(&tmp.path().to_string_lossy());

    v.load(&cfg).unwrap();
}

#[test]
fn load_errors_when_skills_path_not_in_config() {
    let mut v = SkillConfigValidator::new();
    let cfg = config_without_path();

    let err = v.load(&cfg).unwrap_err();
    assert!(
        matches!(err, DomainConfigError::NotFound(_)),
        "got: {err:?}"
    );
}

#[test]
fn validate_before_load_errors() {
    let v = SkillConfigValidator::new();
    let err = v.validate().unwrap_err();
    assert!(
        matches!(err, DomainConfigError::ValidationError { .. }),
        "got: {err:?}"
    );
}

#[test]
fn validate_empty_skills_dir_returns_clean_report() {
    let tmp = tempfile::tempdir().unwrap();
    let mut v = SkillConfigValidator::new();
    let cfg = config_with_path(&tmp.path().to_string_lossy());

    v.load(&cfg).unwrap();
    let report = v.validate().unwrap();
    assert!(
        !report.has_errors(),
        "expected no errors, got: {:?}",
        report.errors
    );
}

#[test]
fn validate_skills_dir_does_not_exist() {
    let mut v = SkillConfigValidator::new();
    let cfg = config_with_path("/nonexistent/skills/path");

    v.load(&cfg).unwrap();
    let report = v.validate().unwrap();
    assert!(report.has_errors(), "expected errors for missing dir");
    let msg = &report.errors[0].message;
    assert!(
        msg.contains("does not exist") || msg.contains("not exist"),
        "got: {msg}"
    );
}

#[test]
fn validate_skill_dir_missing_config_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    let skill_dir = tmp.path().join("my-skill");
    std::fs::create_dir(&skill_dir).unwrap();

    let mut v = SkillConfigValidator::new();
    let cfg = config_with_path(&tmp.path().to_string_lossy());

    v.load(&cfg).unwrap();
    let report = v.validate().unwrap();

    assert!(
        report.has_errors(),
        "expected error for missing config.yaml"
    );
    let msgs: Vec<_> = report.errors.iter().map(|e| e.message.as_str()).collect();
    assert!(
        msgs.iter()
            .any(|m| m.contains("config.yaml") || m.contains("Missing")),
        "got: {msgs:?}"
    );
}

#[test]
fn validate_skill_dir_with_invalid_config_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    let skill_dir = tmp.path().join("bad-skill");
    std::fs::create_dir(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("config.yaml"), b": invalid: yaml::: [[[").unwrap();

    let mut v = SkillConfigValidator::new();
    let cfg = config_with_path(&tmp.path().to_string_lossy());

    v.load(&cfg).unwrap();
    let report = v.validate().unwrap();

    assert!(
        report.has_errors(),
        "expected error for invalid config.yaml"
    );
}

#[test]
fn validate_skill_dir_missing_content_file() {
    let tmp = tempfile::tempdir().unwrap();
    let skill_dir = tmp.path().join("my-skill");
    std::fs::create_dir(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("config.yaml"), valid_skill_yaml()).unwrap();

    let mut v = SkillConfigValidator::new();
    let cfg = config_with_path(&tmp.path().to_string_lossy());

    v.load(&cfg).unwrap();
    let report = v.validate().unwrap();

    assert!(
        report.has_errors(),
        "expected error for missing content file"
    );
    let msgs: Vec<_> = report.errors.iter().map(|e| e.message.as_str()).collect();
    assert!(
        msgs.iter()
            .any(|m| m.contains("not found") || m.contains("Content file")),
        "got: {msgs:?}"
    );
}

#[test]
fn validate_skill_dir_with_valid_skill() {
    let tmp = tempfile::tempdir().unwrap();
    let skill_dir = tmp.path().join("good-skill");
    std::fs::create_dir(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("config.yaml"), valid_skill_yaml()).unwrap();
    std::fs::write(skill_dir.join("index.md"), b"# My Skill content").unwrap();

    let mut v = SkillConfigValidator::new();
    let cfg = config_with_path(&tmp.path().to_string_lossy());

    v.load(&cfg).unwrap();
    let report = v.validate().unwrap();

    assert!(
        !report.has_errors(),
        "expected no errors, got: {:?}",
        report.errors
    );
}

#[test]
fn validate_non_directory_entries_are_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("readme.txt"), b"not a skill dir").unwrap();

    let mut v = SkillConfigValidator::new();
    let cfg = config_with_path(&tmp.path().to_string_lossy());

    v.load(&cfg).unwrap();
    let report = v.validate().unwrap();

    assert!(
        !report.has_errors(),
        "files should be skipped: {:?}",
        report.errors
    );
}

#[test]
fn validate_mixed_valid_and_invalid_skills() {
    let tmp = tempfile::tempdir().unwrap();

    let good_dir = tmp.path().join("good-skill");
    std::fs::create_dir(&good_dir).unwrap();
    std::fs::write(good_dir.join("config.yaml"), valid_skill_yaml()).unwrap();
    std::fs::write(good_dir.join("index.md"), b"content").unwrap();

    let bad_dir = tmp.path().join("bad-skill");
    std::fs::create_dir(&bad_dir).unwrap();

    let mut v = SkillConfigValidator::new();
    let cfg = config_with_path(&tmp.path().to_string_lossy());

    v.load(&cfg).unwrap();
    let report = v.validate().unwrap();

    assert!(report.has_errors(), "expected errors from bad-skill");
}
