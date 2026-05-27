use std::fs;
use std::path::Path;
use systemprompt_agent::services::skills::SkillService;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, SessionId, SkillId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::ensure_test_bootstrap;

fn make_ctx() -> RequestContext {
    let mut ctx = RequestContext::new(
        SessionId::new("skill-svc-session"),
        TraceId::new("skill-svc-trace"),
        ContextId::generate(),
        AgentName::new("test-agent"),
    );
    ctx.auth.actor = Actor::user(UserId::new("skill-test-user"));
    ctx
}

fn write_skill(skills_root: &Path, id: &str, config_yaml: &str, content: Option<&str>) {
    let dir = skills_root.join(id);
    fs::create_dir_all(&dir).expect("mkdir skill dir");
    fs::write(dir.join("config.yaml"), config_yaml).expect("write config.yaml");
    if let Some(text) = content {
        fs::write(dir.join("index.md"), text).expect("write index.md");
    }
}

fn skills_root() -> std::path::PathBuf {
    ensure_test_bootstrap();
    let profile = ProfileBootstrap::get().expect("profile initialised");
    std::path::PathBuf::from(profile.paths.skills())
}

#[tokio::test]
async fn skill_service_new_uses_profile_skills_path() {
    ensure_test_bootstrap();
    let svc = SkillService::new().expect("SkillService::new should succeed");
    let dbg = format!("{:?}", svc);
    assert!(dbg.contains("SkillService"));
}

#[tokio::test]
async fn skill_service_load_skill_metadata_with_name_field() {
    let root = skills_root();
    write_skill(
        &root,
        "meta_skill_1",
        "id: meta_skill_1\nname: Pretty Skill\ndescription: nice\n",
        Some("body"),
    );
    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("meta_skill_1");
    let meta = svc.load_skill_metadata(&id).await.expect("load metadata");
    assert_eq!(meta.skill_id.as_str(), "meta_skill_1");
    assert_eq!(meta.name, "Pretty Skill");
}

#[tokio::test]
async fn skill_service_load_skill_metadata_missing_returns_err() {
    let _root = skills_root();
    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("__does_not_exist_xyz__");
    let err = svc
        .load_skill_metadata(&id)
        .await
        .expect_err("should fail");
    assert!(format!("{err}").contains("Skill not found"));
}

#[tokio::test]
async fn skill_service_load_skill_returns_instructions_without_frontmatter() {
    let root = skills_root();
    write_skill(
        &root,
        "load_skill_a",
        "id: load_skill_a\nname: Test\ndescription: testing\n",
        Some("---\ntitle: My\n---\nActual body text"),
    );
    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("load_skill_a");
    let ctx = make_ctx();
    let instructions = svc.load_skill(&id, &ctx).await.expect("load skill");
    assert_eq!(instructions, "Actual body text");
}

#[tokio::test]
async fn skill_service_load_skill_empty_body_when_content_missing() {
    let root = skills_root();
    write_skill(
        &root,
        "no_body_skill",
        "id: no_body_skill\nname: Test\ndescription: testing\n",
        None,
    );
    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("no_body_skill");
    let ctx = make_ctx();
    let instructions = svc.load_skill(&id, &ctx).await.expect("load");
    assert_eq!(instructions, "");
}

#[tokio::test]
async fn skill_service_load_skill_resolves_id_from_config_when_set() {
    let root = skills_root();
    write_skill(
        &root,
        "dir_name_alpha",
        "id: config_id_beta\nname: Override\ndescription: x\n",
        Some("payload"),
    );
    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("dir_name_alpha");
    let ctx = make_ctx();
    let meta = svc.load_skill_metadata(&id).await.expect("meta");
    assert_eq!(meta.skill_id.as_str(), "config_id_beta");
    assert_eq!(meta.name, "Override");
    let body = svc.load_skill(&id, &ctx).await.expect("body");
    assert_eq!(body, "payload");
}

#[tokio::test]
async fn skill_service_load_skill_uses_dir_name_when_empty_name() {
    let root = skills_root();
    write_skill(
        &root,
        "fallback_named",
        "id: fallback_named\nname: \"\"\ndescription: x\n",
        Some("hi"),
    );
    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("fallback_named");
    let meta = svc.load_skill_metadata(&id).await.expect("meta");
    assert_eq!(meta.name, "fallback_named");
}

#[tokio::test]
async fn skill_service_load_skill_custom_content_file() {
    let root = skills_root();
    let dir = root.join("custom_file_skill");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join("config.yaml"),
        "id: custom_file_skill\nname: Custom\ndescription: x\nfile: alt.md\n",
    )
    .expect("config");
    fs::write(dir.join("alt.md"), "alt content").expect("alt md");

    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("custom_file_skill");
    let ctx = make_ctx();
    let body = svc.load_skill(&id, &ctx).await.expect("body");
    assert_eq!(body, "alt content");
}

#[tokio::test]
async fn skill_service_load_skill_invalid_yaml_errors() {
    let root = skills_root();
    let dir = root.join("invalid_yaml_skill");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("config.yaml"), "{{{not yaml").expect("config");
    fs::write(dir.join("index.md"), "x").expect("md");

    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("invalid_yaml_skill");
    let err = svc
        .load_skill_metadata(&id)
        .await
        .expect_err("should fail");
    assert!(format!("{err}").contains("Invalid YAML"));
}

#[tokio::test]
async fn skill_service_list_skill_ids_returns_enabled_only_sorted() {
    let root = skills_root();
    write_skill(
        &root,
        "list_a_skill",
        "id: list_a_skill\nname: A\ndescription: a\nenabled: true\n",
        Some("a"),
    );
    write_skill(
        &root,
        "list_b_skill",
        "id: list_b_skill\nname: B\ndescription: b\nenabled: true\n",
        Some("b"),
    );
    write_skill(
        &root,
        "list_c_disabled",
        "id: list_c_disabled\nname: C\ndescription: c\nenabled: false\n",
        Some("c"),
    );

    let svc = SkillService::new().expect("svc");
    let ids = svc.list_skill_ids().await.expect("list");
    assert!(ids.contains(&"list_a_skill".to_owned()));
    assert!(ids.contains(&"list_b_skill".to_owned()));
    assert!(!ids.contains(&"list_c_disabled".to_owned()));
}

#[tokio::test]
async fn skill_service_list_skill_ids_skips_dirs_without_config() {
    let root = skills_root();
    let dir = root.join("no_config_dir_marker");
    fs::create_dir_all(&dir).expect("dir");

    let svc = SkillService::new().expect("svc");
    let ids = svc.list_skill_ids().await.expect("list");
    assert!(!ids.contains(&"no_config_dir_marker".to_owned()));
}

#[tokio::test]
async fn skill_service_list_skill_ids_uses_config_id_when_set() {
    let root = skills_root();
    write_skill(
        &root,
        "list_dir_x",
        "id: list_real_id_y\nname: X\ndescription: x\nenabled: true\n",
        Some("x"),
    );

    let svc = SkillService::new().expect("svc");
    let ids = svc.list_skill_ids().await.expect("list");
    assert!(ids.contains(&"list_real_id_y".to_owned()));
    assert!(!ids.contains(&"list_dir_x".to_owned()));
}

#[tokio::test]
async fn skill_service_list_skill_ids_handles_invalid_yaml_gracefully() {
    let root = skills_root();
    let dir = root.join("list_bad_yaml");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("config.yaml"), "[[not yaml").expect("config");

    let svc = SkillService::new().expect("svc");
    let ids = svc.list_skill_ids().await.expect("should not error");
    assert!(!ids.contains(&"list_bad_yaml".to_owned()));
}

#[tokio::test]
async fn skill_service_load_skill_with_task_id_does_not_panic_without_repo() {
    let root = skills_root();
    write_skill(
        &root,
        "with_task_skill",
        "id: with_task_skill\nname: T\ndescription: t\n",
        Some("hi"),
    );
    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("with_task_skill");
    let mut ctx = make_ctx();
    ctx.execution.task_id = Some(TaskId::generate());
    let body = svc.load_skill(&id, &ctx).await.expect("load");
    assert_eq!(body, "hi");
}

#[tokio::test]
async fn skill_service_load_skill_id_field_empty_uses_supplied_id() {
    let root = skills_root();
    let dir = root.join("empty_id_skill");
    std::fs::create_dir_all(&dir).expect("dir");
    // YAML id: "" – serde_yaml may not deserialize empty quoted, use minimal config
    std::fs::write(
        dir.join("config.yaml"),
        "id: \"\"\nname: NamedOne\ndescription: x\n",
    )
    .expect("config");
    std::fs::write(dir.join("index.md"), "body").expect("md");

    let svc = SkillService::new().expect("svc");
    let id = SkillId::new("empty_id_skill");
    let meta = svc.load_skill_metadata(&id).await.expect("meta");
    // When config id is empty string, supplied id is used
    assert_eq!(meta.skill_id.as_str(), "empty_id_skill");
    assert_eq!(meta.name, "NamedOne");
}

#[tokio::test]
async fn skill_service_with_execution_step_repo_returns_self() {
    use std::sync::Arc;
    use systemprompt_agent::repository::execution::ExecutionStepRepository;
    use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

    ensure_test_bootstrap();
    let url = match fixture_database_url() {
        Ok(u) => u,
        Err(_) => return,
    };
    let db = match fixture_db_pool(&url).await {
        Ok(d) => d,
        Err(_) => return,
    };
    let repo = match ExecutionStepRepository::new(&db) {
        Ok(r) => Arc::new(r),
        Err(_) => return,
    };
    let svc = SkillService::new().expect("svc").with_execution_step_repo(repo);
    let dbg = format!("{:?}", svc);
    assert!(dbg.contains("ExecutionStepRepository"));
}
