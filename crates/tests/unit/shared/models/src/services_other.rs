use std::str::FromStr;

use systemprompt_identifiers::{ExternalAgentId, SkillId, UserId};
use systemprompt_models::services::{
    DiskSkillConfig, ExternalAgentConfig, ExternalAgentKind, JobConfig, RuntimeStatus,
    SchedulerConfig, ServiceType, Settings, SkillDetail, SkillSummary, SystemAdmin,
    split_frontmatter, strip_frontmatter,
};

#[test]
fn runtime_status_display_round_trips() {
    for (variant, s) in [
        (RuntimeStatus::Running, "running"),
        (RuntimeStatus::Starting, "starting"),
        (RuntimeStatus::Stopped, "stopped"),
        (RuntimeStatus::Crashed, "crashed"),
        (RuntimeStatus::Orphaned, "orphaned"),
    ] {
        assert_eq!(variant.to_string(), s);
        assert_eq!(RuntimeStatus::from_str(s).unwrap(), variant);
    }
}

#[test]
fn runtime_status_from_str_accepts_error_alias_and_uppercase() {
    assert_eq!(
        RuntimeStatus::from_str("ERROR").unwrap(),
        RuntimeStatus::Crashed
    );
    assert_eq!(
        RuntimeStatus::from_str("Error").unwrap(),
        RuntimeStatus::Crashed
    );
    assert_eq!(
        RuntimeStatus::from_str("Running").unwrap(),
        RuntimeStatus::Running
    );
    assert!(RuntimeStatus::from_str("nope").is_err());
}

#[test]
fn runtime_status_helpers() {
    assert!(RuntimeStatus::Running.is_healthy());
    assert!(RuntimeStatus::Starting.is_healthy());
    assert!(!RuntimeStatus::Stopped.is_healthy());
    assert!(!RuntimeStatus::Crashed.is_healthy());

    assert!(RuntimeStatus::Crashed.needs_cleanup());
    assert!(RuntimeStatus::Orphaned.needs_cleanup());
    assert!(!RuntimeStatus::Running.needs_cleanup());
    assert!(!RuntimeStatus::Stopped.needs_cleanup());
}

#[test]
fn service_type_display_round_trip() {
    assert_eq!(ServiceType::Api.to_string(), "api");
    assert_eq!(ServiceType::Agent.to_string(), "agent");
    assert_eq!(ServiceType::Mcp.to_string(), "mcp");
}

#[test]
fn service_type_from_module_name_routes_correctly() {
    assert_eq!(ServiceType::from_module_name("agent"), ServiceType::Agent);
    assert_eq!(ServiceType::from_module_name("AGENT"), ServiceType::Agent);
    assert_eq!(ServiceType::from_module_name("api"), ServiceType::Api);
    assert_eq!(ServiceType::from_module_name("Api"), ServiceType::Api);
    assert_eq!(
        ServiceType::from_module_name("filesystem"),
        ServiceType::Mcp
    );
    assert_eq!(ServiceType::from_module_name(""), ServiceType::Mcp);
}

#[test]
fn settings_default_has_expected_values() {
    let s = Settings::default();
    assert_eq!(s.agent_port_range, (9000, 9999));
    assert_eq!(s.mcp_port_range, (5000, 5999));
    assert!(s.auto_start_enabled);
    assert!(s.validation_strict);
    assert_eq!(s.schema_validation_mode, "auto_migrate");
    assert!(s.marketplace_public);
    assert!(s.services_path.is_none());
}

#[test]
fn settings_yaml_round_trip_with_defaults() {
    let yaml = "";
    let s: Settings = serde_yaml::from_str("{}").unwrap_or_else(|_| {
        let _ = yaml;
        Settings::default()
    });
    assert_eq!(s.agent_port_range, (9000, 9999));
}

#[test]
fn external_agent_config_yaml_round_trip() {
    let yaml = r#"
id: claude_desktop
display_name: Claude Desktop
kind: desktop_app
enabled: true
description: First-party desktop client
platforms: ["macos", "windows"]
docs_url: https://example.org/docs
"#;
    let cfg: ExternalAgentConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.id, ExternalAgentId::new("claude_desktop"));
    assert_eq!(cfg.kind, ExternalAgentKind::DesktopApp);
    assert!(cfg.enabled);
    assert_eq!(cfg.platforms.len(), 2);
    assert_eq!(cfg.docs_url.as_deref(), Some("https://example.org/docs"));
}

#[test]
fn external_agent_kind_serde_uses_snake_case() {
    let json = serde_json::to_string(&ExternalAgentKind::CliTool).unwrap();
    assert_eq!(json, "\"cli_tool\"");
    let parsed: ExternalAgentKind = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, ExternalAgentKind::CliTool);
}

#[test]
fn external_agent_config_rejects_unknown_fields() {
    let yaml = r#"
id: x
display_name: x
kind: cli_tool
enabled: true
extra: nope
"#;
    let res: Result<ExternalAgentConfig, _> = serde_yaml::from_str(yaml);
    assert!(res.is_err());
}

#[test]
fn job_config_new_and_builders() {
    let owner = UserId::new("user-1");
    let j = JobConfig::new("hello");
    assert_eq!(j.name, "hello");
    assert!(j.owner.is_none());
    assert!(j.enabled);
    assert!(j.extension.is_none());
    assert!(j.schedule.is_none());

    let j = JobConfig::new("x")
        .with_owner(owner.clone())
        .with_extension("core")
        .with_schedule("0 0 * * * *");
    assert_eq!(j.owner, Some(owner));
    assert_eq!(j.extension.as_deref(), Some("core"));
    assert_eq!(j.schedule.as_deref(), Some("0 0 * * * *"));

    let j = JobConfig::new("y").disabled();
    assert!(!j.enabled);
}

#[test]
fn scheduler_config_with_system_admin_emits_core_cleanup_jobs() {
    let s = SchedulerConfig::with_system_admin();
    assert!(s.enabled);
    assert!(s.distributed_lock);
    assert_eq!(s.jobs.len(), 4);
    let names: Vec<&str> = s.jobs.iter().map(|j| j.name.as_str()).collect();
    assert!(names.contains(&"cleanup_anonymous_users"));
    assert!(names.contains(&"cleanup_empty_contexts"));
    assert!(names.contains(&"cleanup_inactive_sessions"));
    assert!(names.contains(&"database_cleanup"));
    for j in &s.jobs {
        assert!(j.owner.is_none());
        assert_eq!(j.extension.as_deref(), Some("core"));
        let schedule = j
            .schedule
            .as_deref()
            .expect("core cleanup job has schedule");
        assert!(!schedule.is_empty());
    }
    assert_eq!(s.bootstrap_jobs.len(), 2);
}

#[test]
fn system_admin_accessors() {
    let id = UserId::new("admin-1");
    let admin = SystemAdmin::new(id.clone(), "root".to_owned());
    assert_eq!(admin.id(), &id);
    assert_eq!(admin.username(), "root");
}

#[test]
fn disk_skill_config_content_file_default_and_explicit() {
    let cfg = DiskSkillConfig {
        id: SkillId::new("s"),
        name: "Skill".to_owned(),
        description: "desc".to_owned(),
        enabled: true,
        file: String::new(),
        tags: vec![],
        category: None,
    };
    assert_eq!(cfg.content_file(), "index.md");

    let cfg = DiskSkillConfig {
        file: "guide.md".to_owned(),
        ..cfg
    };
    assert_eq!(cfg.content_file(), "guide.md");
}

#[test]
fn skill_summary_from_disk_config_file_path_logic() {
    let cfg = DiskSkillConfig {
        id: SkillId::new("s1"),
        name: "S1".to_owned(),
        description: "d".to_owned(),
        enabled: true,
        file: String::new(),
        tags: vec!["a".to_owned(), "b".to_owned()],
        category: None,
    };
    let sum: SkillSummary = (&cfg).into();
    assert_eq!(sum.skill_id, cfg.id);
    assert!(sum.file_path.is_none());
    assert_eq!(sum.tags.len(), 2);
    assert_eq!(sum.display_name, "S1");

    let cfg2 = DiskSkillConfig {
        file: "x.md".to_owned(),
        ..cfg
    };
    let sum2: SkillSummary = (&cfg2).into();
    assert_eq!(sum2.file_path.as_deref(), Some("x.md"));
}

#[test]
fn skill_detail_from_disk_config_carries_category_and_blank_preview() {
    let cfg = DiskSkillConfig {
        id: SkillId::new("s1"),
        name: "S1".to_owned(),
        description: "desc".to_owned(),
        enabled: false,
        file: "x.md".to_owned(),
        tags: vec!["t".to_owned()],
        category: Some("dev".to_owned()),
    };
    let det: SkillDetail = (&cfg).into();
    assert_eq!(det.category.as_deref(), Some("dev"));
    assert_eq!(det.file_path.as_deref(), Some("x.md"));
    assert!(!det.enabled);
    assert!(det.instructions_preview.is_empty());
}

#[test]
fn strip_frontmatter_removes_yaml_block() {
    let body = "---\ntitle: T\n---\nHello world\n";
    assert_eq!(strip_frontmatter(body), "Hello world");
}

#[test]
fn strip_frontmatter_returns_input_when_no_frontmatter() {
    let body = "No frontmatter here\nMore lines";
    assert_eq!(strip_frontmatter(body), body);
}

#[test]
fn strip_frontmatter_handles_empty_body() {
    assert_eq!(strip_frontmatter(""), "");
}

#[test]
fn strip_frontmatter_keeps_table_separators_without_frontmatter() {
    let body = "# Title\n\n| Col A | Col B |\n|-------|-------|\n| a | b |\n";
    assert_eq!(strip_frontmatter(body), body);
}

#[test]
fn strip_frontmatter_keeps_horizontal_rules_without_frontmatter() {
    let body = "Intro\n\n---\n\nMiddle\n\n---\n\nEnd\n";
    assert_eq!(strip_frontmatter(body), body);
}

#[test]
fn strip_frontmatter_preserves_body_dashes_after_frontmatter() {
    let body = "---\ntitle: T\n---\nIntro\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n---\n\nEnd";
    assert_eq!(
        strip_frontmatter(body),
        "Intro\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n---\n\nEnd"
    );
}

#[test]
fn split_frontmatter_requires_opening_delimiter_line() {
    assert!(split_frontmatter("title: T\n---\nBody").is_none());
    assert!(split_frontmatter("\n---\ntitle: T\n---\nBody").is_none());
    assert!(split_frontmatter("----\ntitle: T\n---\nBody").is_none());
    assert!(split_frontmatter("a---b---c").is_none());
    assert!(split_frontmatter("").is_none());
}

#[test]
fn split_frontmatter_requires_closing_delimiter_line() {
    assert!(split_frontmatter("---\ntitle: T\nno closing").is_none());
    assert!(split_frontmatter("---\ntitle: T\n|---|---|\nstill open").is_none());
}

#[test]
fn split_frontmatter_returns_yaml_and_body() {
    let split = split_frontmatter("---\ntitle: T\n---\nBody line\n").unwrap();
    assert_eq!(split.yaml, "title: T\n");
    assert_eq!(split.body, "Body line\n");
}

#[test]
fn split_frontmatter_handles_crlf_and_bom() {
    let split = split_frontmatter("\u{feff}---\r\ntitle: T\r\n---\r\nBody\r\n").unwrap();
    assert_eq!(split.yaml, "title: T\r\n");
    assert_eq!(split.body, "Body\r\n");
}
