use std::fs;
use std::path::Path;
use std::sync::Arc;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::services::skills::SkillService;
use systemprompt_config::ProfileBootstrap;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, SessionId, SkillId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::{
    ScriptedSkills, ensure_test_bootstrap, not_managed_skills, scripted_skills,
};
use systemprompt_test_mocks::recording_webhooks;
use systemprompt_traits::{DynManagedSkillResolver, ResolvedManagedSkill, WithheldReason};

fn make_ctx() -> RequestContext {
    let mut ctx = RequestContext::new(
        SessionId::new("skill-svc-session"),
        TraceId::new("skill-svc-trace"),
        ContextId::generate(),
        AgentName::try_new("test-agent").expect("valid AgentName"),
    );
    ctx.auth.actor = Actor::user(UserId::new("skill-test-user"));
    ctx
}

fn owner() -> UserId {
    UserId::new("skill-test-user")
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

fn service_with(pool: &DbPool, managed: DynManagedSkillResolver) -> SkillService {
    ensure_test_bootstrap();
    let repo = Arc::new(ExecutionStepRepository::new(pool).expect("step repo"));
    SkillService::new(managed, repo, recording_webhooks()).expect("skill service")
}

async fn disk_service() -> Option<(DbPool, SkillService)> {
    let pool = crate::repository::try_pool_or_skip().await?;
    let svc = service_with(&pool, not_managed_skills());
    Some((pool, svc))
}

#[tokio::test]
async fn skill_service_load_skill_metadata_with_name_field() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    write_skill(
        &root,
        "meta_skill_1",
        "id: meta_skill_1\nname: Pretty Skill\ndescription: nice\n",
        Some("body"),
    );
    let id = SkillId::new("meta_skill_1");
    let meta = svc
        .load_skill_metadata(&id, &owner())
        .await
        .expect("load metadata");
    assert_eq!(meta.skill_id.as_str(), "meta_skill_1");
    assert_eq!(meta.name, "Pretty Skill");
}

#[tokio::test]
async fn skill_service_load_skill_metadata_missing_returns_err() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let _root = skills_root();
    let id = SkillId::new("__does_not_exist_xyz__");
    let err = svc
        .load_skill_metadata(&id, &owner())
        .await
        .expect_err("should fail");
    assert!(format!("{err}").contains("Skill not found"));
}

#[tokio::test]
async fn skill_service_load_skill_returns_instructions_without_frontmatter() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    write_skill(
        &root,
        "load_skill_a",
        "id: load_skill_a\nname: Test\ndescription: testing\n",
        Some("---\ntitle: My\n---\nActual body text"),
    );
    let id = SkillId::new("load_skill_a");
    let ctx = make_ctx();
    let instructions = svc.load_skill(&id, &ctx).await.expect("load skill");
    assert_eq!(instructions, "Actual body text");
}

#[tokio::test]
async fn skill_service_load_skill_empty_body_when_content_missing() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    write_skill(
        &root,
        "no_body_skill",
        "id: no_body_skill\nname: Test\ndescription: testing\n",
        None,
    );
    let id = SkillId::new("no_body_skill");
    let ctx = make_ctx();
    let instructions = svc.load_skill(&id, &ctx).await.expect("load");
    assert_eq!(instructions, "");
}

#[tokio::test]
async fn skill_service_load_skill_resolves_id_from_config_when_set() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    write_skill(
        &root,
        "dir_name_alpha",
        "id: config_id_beta\nname: Override\ndescription: x\n",
        Some("payload"),
    );
    let id = SkillId::new("dir_name_alpha");
    let ctx = make_ctx();
    let meta = svc.load_skill_metadata(&id, &owner()).await.expect("meta");
    assert_eq!(meta.skill_id.as_str(), "config_id_beta");
    assert_eq!(meta.name, "Override");
    let body = svc.load_skill(&id, &ctx).await.expect("body");
    assert_eq!(body, "payload");
}

#[tokio::test]
async fn skill_service_load_skill_uses_dir_name_when_empty_name() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    write_skill(
        &root,
        "fallback_named",
        "id: fallback_named\nname: \"\"\ndescription: x\n",
        Some("hi"),
    );
    let id = SkillId::new("fallback_named");
    let meta = svc.load_skill_metadata(&id, &owner()).await.expect("meta");
    assert_eq!(meta.name, "fallback_named");
}

#[tokio::test]
async fn skill_service_load_skill_custom_content_file() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    let dir = root.join("custom_file_skill");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join("config.yaml"),
        "id: custom_file_skill\nname: Custom\ndescription: x\nfile: alt.md\n",
    )
    .expect("config");
    fs::write(dir.join("alt.md"), "alt content").expect("alt md");

    let id = SkillId::new("custom_file_skill");
    let ctx = make_ctx();
    let body = svc.load_skill(&id, &ctx).await.expect("body");
    assert_eq!(body, "alt content");
}

#[tokio::test]
async fn skill_service_load_skill_invalid_yaml_errors() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let _skills_fixture_write = crate::SKILLS_FIXTURE_LOCK.write().await;
    let root = skills_root();
    let dir = root.join("invalid_yaml_skill");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("config.yaml"), "{{{not yaml").expect("config");
    fs::write(dir.join("index.md"), "x").expect("md");

    let id = SkillId::new("invalid_yaml_skill");
    let result = svc.load_skill_metadata(&id, &owner()).await;
    // Drop the malformed stub before yielding: sibling `registry_service`
    // tests load this shared dir in full via the strict `ConfigLoader`, which
    // (unlike a targeted `load_skill_metadata`) rejects an unparseable stub.
    fs::remove_dir_all(&dir).ok();
    let err = result.expect_err("should fail");
    assert!(format!("{err}").contains("Invalid YAML"));
}

#[tokio::test]
async fn skill_service_load_skill_id_field_empty_uses_supplied_id() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    let dir = root.join("empty_id_skill");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(
        dir.join("config.yaml"),
        "id: \"\"\nname: NamedOne\ndescription: x\n",
    )
    .expect("config");
    std::fs::write(dir.join("index.md"), "body").expect("md");

    let id = SkillId::new("empty_id_skill");
    let meta = svc.load_skill_metadata(&id, &owner()).await.expect("meta");
    assert_eq!(meta.skill_id.as_str(), "empty_id_skill");
    assert_eq!(meta.name, "NamedOne");
}

fn ctx_with_task(context_id: &ContextId, task_id: &TaskId) -> RequestContext {
    let mut ctx = RequestContext::new(
        SessionId::new("skill-track-session"),
        TraceId::new("skill-track-trace"),
        context_id.clone(),
        AgentName::try_new("test-agent").expect("valid AgentName"),
    );
    ctx.auth.actor = Actor::user(UserId::new("skill-track-user"));
    ctx.with_task_id(task_id.clone())
}

#[tokio::test]
async fn load_skill_without_a_task_id_still_returns_instructions() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    let id = format!("notrack{}", uuid::Uuid::new_v4().simple());
    write_skill(
        &root,
        &id,
        &format!("id: {id}\nname: No Track\ndescription: d\n"),
        Some("Do the thing.\n"),
    );

    let instructions = svc
        .load_skill(&SkillId::new(&id), &make_ctx())
        .await
        .expect("load should succeed even with nothing to track against");

    assert!(
        instructions.contains("Do the thing."),
        "instructions should come back verbatim: {instructions}"
    );
}

#[tokio::test]
async fn load_skill_records_an_execution_step_for_the_task() {
    let Some(pool) = crate::repository::try_pool_or_skip().await else {
        return;
    };
    let repos = crate::repository::repos(&pool);
    let (user, session) = crate::repository::seed_user_and_session(&pool).await;
    let (context_id, task_id) =
        crate::repository::seed_context_and_task(&repos, &user, &session).await;

    let root = skills_root();
    let id = format!("tracked{}", uuid::Uuid::new_v4().simple());
    write_skill(
        &root,
        &id,
        &format!("id: {id}\nname: Tracked Skill\ndescription: d\n"),
        Some("Tracked body.\n"),
    );

    let step_repo = Arc::new(ExecutionStepRepository::new(&pool).expect("step repo"));
    let svc = SkillService::new(
        not_managed_skills(),
        Arc::clone(&step_repo),
        recording_webhooks(),
    )
    .expect("service");

    let instructions = svc
        .load_skill(&SkillId::new(&id), &ctx_with_task(&context_id, &task_id))
        .await
        .expect("load should succeed");
    assert!(instructions.contains("Tracked body."));

    let steps = step_repo
        .list_by_task(&task_id)
        .await
        .expect("steps should be readable");
    assert!(
        steps.iter().any(|s| format!("{s:?}").contains(&id)),
        "loading a skill must record a step naming it: {steps:?}"
    );
}

#[tokio::test]
async fn a_published_managed_skill_is_served_without_touching_the_disk_catalogue() {
    let Some(pool) = crate::repository::try_pool_or_skip().await else {
        return;
    };
    let _root = skills_root();
    let published = ResolvedManagedSkill {
        id: SkillId::new("managed-only"),
        name: "Managed Only".to_owned(),
        description: "served by the authority".to_owned(),
        instructions: "Managed instructions.".to_owned(),
    };
    let svc = service_with(&pool, scripted_skills(ScriptedSkills::Published(published)));

    let instructions = svc
        .load_skill(&SkillId::new("managed-only"), &make_ctx())
        .await
        .expect("published skill loads");
    assert_eq!(instructions, "Managed instructions.");

    let meta = svc
        .load_skill_metadata(&SkillId::new("managed-only"), &owner())
        .await
        .expect("metadata comes from the authority");
    assert_eq!(meta.name, "Managed Only");
}

#[tokio::test]
async fn a_withheld_managed_skill_is_an_error_even_when_a_disk_copy_exists() {
    let Some(pool) = crate::repository::try_pool_or_skip().await else {
        return;
    };
    let root = skills_root();
    write_skill(
        &root,
        "withheld_skill",
        "id: withheld_skill\nname: Withheld\ndescription: d\n",
        Some("disk copy"),
    );
    let svc = service_with(
        &pool,
        scripted_skills(ScriptedSkills::Withheld(WithheldReason::Withdrawn)),
    );

    let err = svc
        .load_skill(&SkillId::new("withheld_skill"), &make_ctx())
        .await
        .expect_err("withheld must not fall back to disk");
    assert!(err.to_string().contains("withheld"), "{err}");
    assert!(err.to_string().contains("withdrawn"), "{err}");

    let meta_err = svc
        .load_skill_metadata(&SkillId::new("withheld_skill"), &owner())
        .await
        .expect_err("metadata goes through the same authority");
    assert!(meta_err.to_string().contains("withheld"), "{meta_err}");
}

#[tokio::test]
async fn an_unavailable_authority_fails_the_load() {
    let Some(pool) = crate::repository::try_pool_or_skip().await else {
        return;
    };
    let root = skills_root();
    write_skill(
        &root,
        "outage_skill",
        "id: outage_skill\nname: Outage\ndescription: d\n",
        Some("disk copy"),
    );
    let svc = service_with(&pool, scripted_skills(ScriptedSkills::Unavailable));

    let err = svc
        .load_skill(&SkillId::new("outage_skill"), &make_ctx())
        .await
        .expect_err("an authority outage is not a disk fallback");
    assert!(err.to_string().contains("outage"), "{err}");
}

#[test]
fn coverage_skill_service_requires_a_profile_before_loading_disk_content() {
    assert!(ProfileBootstrap::get().is_err());
    let Ok(url) = systemprompt_test_fixtures::fixture_database_url() else {
        return;
    };
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let Ok(pool) = rt.block_on(systemprompt_test_fixtures::fixture_db_pool(&url)) else {
        return;
    };
    let repo = Arc::new(ExecutionStepRepository::new(&pool).expect("step repo"));
    let err = SkillService::new(not_managed_skills(), repo, recording_webhooks()).unwrap_err();
    assert!(err.to_string().contains("Profile not initialized"));
}

#[tokio::test]
async fn coverage_skill_config_read_failure_names_the_file() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    fs::create_dir_all(root.join("blocked/config.yaml")).unwrap();
    let err = svc
        .load_skill_metadata(&SkillId::new("blocked"), &owner())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Failed to read"));
    assert!(err.to_string().contains("config.yaml"));
}

#[tokio::test]
async fn coverage_skill_content_read_failure_is_not_empty_instructions() {
    let Some((_pool, svc)) = disk_service().await else {
        return;
    };
    let root = skills_root();
    write_skill(
        &root,
        "blocked_body",
        "id: blocked_body\nname: Blocked\ndescription: blocked content\n",
        None,
    );
    fs::create_dir(root.join("blocked_body/index.md")).unwrap();
    let err = svc
        .load_skill(&SkillId::new("blocked_body"), &make_ctx())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("index.md"));
}
