//! Unit tests for systemprompt-core-agent crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/domain/agent/src/error.rs`
//! - Test: `crates/tests/unit/domain/agent/src/error.rs`
//!
//! Tests cover:
//! - Error types (ArtifactError, AgentError, AgentServiceError)
//! - Models (skill, context, runtime, a2a protocol, web models)
//! - Services (shared utilities, agent orchestration)

// Serialises the deliberately-corrupt skill fixture writer in
// `services::skills::skill_service` against the strict services loader
// readers in `services::registry::registry_service`. Both scan the
// process-shared bootstrap skills dir; the strict loader rejects the malformed
// stub the lenient `list_skill_ids` test stages, so the writer must hold this
// exclusively while the bad file is on disk.
#[cfg(test)]
pub(crate) static SKILLS_FIXTURE_LOCK: tokio::sync::RwLock<()> = tokio::sync::RwLock::const_new(());


#[cfg(test)]
pub(crate) fn card_site(
    api_external_url: &str,
) -> systemprompt_agent::services::registry::CardSite<'_> {
    static SKILLS_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    systemprompt_agent::services::registry::CardSite {
        api_external_url,
        skills_dir: SKILLS_DIR.get_or_init(systemprompt_test_fixtures::test_skills_root),
    }
}

#[cfg(test)]
pub(crate) fn spawn_environment()
-> std::sync::Arc<systemprompt_agent::services::agent_orchestration::process::SpawnEnvironment> {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    std::sync::Arc::new(
        systemprompt_agent::services::agent_orchestration::process::SpawnEnvironment {
            config: std::sync::Arc::clone(bootstrap.boot_config()),
            secrets: std::sync::Arc::clone(bootstrap.boot.secrets()),
            profile_path: bootstrap.profile_path.clone(),
        },
    )
}

#[cfg(test)]
pub(crate) fn session_usage(
    db: &systemprompt_database::DbPool,
) -> systemprompt_traits::DynSessionUsageCounters {
    systemprompt_test_fixtures::fixture_analytics_repositories(db)
        .expect("session repository")
        .session_store
}

#[cfg(test)]
mod error;

#[cfg(test)]
mod error_extended;

#[cfg(test)]
mod extension;

#[cfg(test)]
mod models;

#[cfg(test)]
mod repository;

#[cfg(test)]
mod services;

#[cfg(test)]
mod state;
