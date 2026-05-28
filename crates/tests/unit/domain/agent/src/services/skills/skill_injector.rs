use std::fs;
use std::path::Path;
use std::sync::Arc;
use systemprompt_agent::services::skills::{SkillInjector, SkillService};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, SkillId, TraceId, UserId};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::ensure_test_bootstrap;

fn make_ctx() -> RequestContext {
    let mut ctx = RequestContext::new(
        SessionId::new("inj-session"),
        TraceId::new("inj-trace"),
        ContextId::generate(),
        AgentName::new("inj-agent"),
    );
    ctx.auth.actor = Actor::user(UserId::new("inj-user"));
    ctx
}

fn write_skill(skills_root: &Path, id: &str, name: &str, body: &str) {
    let dir = skills_root.join(id);
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join("config.yaml"),
        format!("id: {id}\nname: {name}\ndescription: desc\n"),
    )
    .expect("config");
    fs::write(dir.join("index.md"), body).expect("body");
}

fn make_service() -> Arc<SkillService> {
    ensure_test_bootstrap();
    Arc::new(SkillService::new().expect("SkillService::new"))
}

fn skills_root() -> std::path::PathBuf {
    ensure_test_bootstrap();
    std::path::PathBuf::from(ProfileBootstrap::get().unwrap().paths.skills())
}

#[tokio::test]
async fn inject_for_tool_no_skill_returns_base_prompt() {
    let svc = make_service();
    let injector = SkillInjector::new(svc);
    let ctx = make_ctx();
    let out = injector
        .inject_for_tool(None, "BASE".to_string(), &ctx)
        .await
        .expect("inject");
    assert_eq!(out, "BASE");
}

#[tokio::test]
async fn inject_for_tool_with_known_skill_appends_guidance() {
    let root = skills_root();
    write_skill(&root, "inj_known", "Known", "SKILL BODY");
    let svc = make_service();
    let injector = SkillInjector::new(svc);
    let ctx = make_ctx();
    let id = SkillId::new("inj_known");
    let out = injector
        .inject_for_tool(Some(&id), "BASE".to_string(), &ctx)
        .await
        .expect("inject");
    assert!(out.starts_with("BASE"));
    assert!(out.contains("Writing Guidance"));
    assert!(out.contains("SKILL BODY"));
}

#[tokio::test]
async fn inject_for_tool_with_unknown_skill_returns_base() {
    let svc = make_service();
    let injector = SkillInjector::new(svc);
    let ctx = make_ctx();
    let id = SkillId::new("inj_does_not_exist_qq");
    let out = injector
        .inject_for_tool(Some(&id), "BASE".to_string(), &ctx)
        .await
        .expect("inject");
    assert_eq!(out, "BASE");
}

#[tokio::test]
async fn inject_with_metadata_returns_prompt_and_metadata() {
    let root = skills_root();
    write_skill(&root, "inj_meta", "Metadata Skill", "WRITE WELL");
    let svc = make_service();
    let injector = SkillInjector::new(svc);
    let ctx = make_ctx();
    let id = SkillId::new("inj_meta");
    let (prompt, meta) = injector
        .inject_with_metadata(&id, "BASE".to_string(), &ctx)
        .await
        .expect("inject metadata");
    assert!(prompt.contains("WRITE WELL"));
    assert_eq!(meta.skill_id.as_str(), "inj_meta");
    assert_eq!(meta.name, "Metadata Skill");
}

#[tokio::test]
async fn inject_with_metadata_missing_skill_errors() {
    let svc = make_service();
    let injector = SkillInjector::new(svc);
    let ctx = make_ctx();
    let id = SkillId::new("inj_missing_meta_zzz");
    let err = injector
        .inject_with_metadata(&id, "BASE".to_string(), &ctx)
        .await
        .expect_err("should fail");
    assert!(format!("{err}").contains("Skill not found"));
}

#[tokio::test]
async fn get_metadata_returns_known() {
    let root = skills_root();
    write_skill(&root, "inj_get_meta", "GM", "x");
    let svc = make_service();
    let injector = SkillInjector::new(svc);
    let id = SkillId::new("inj_get_meta");
    let meta = injector.get_metadata(&id).await.expect("metadata");
    assert_eq!(meta.name, "GM");
}
