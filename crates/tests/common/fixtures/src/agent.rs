//! Agent-domain collaborators for tests: the seams `A2ARepositories`
//! requires, in their simplest deterministic shapes. The recording webhook
//! broadcaster lives in `systemprompt-test-mocks`.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_agent::repository::{A2ARepositories, A2aDependencies};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{InstanceId, McpExecutionId, UserId};
use systemprompt_traits::{
    DynManagedSkillResolver, DynToolExecutionLookup, ManagedSkillResolver,
    ManagedSkillResolverError, RepositoryError, ResolvedManagedSkill, SkillResolution,
    ToolExecutionLookup, WithheldReason,
};

/// A managed-skill authority that manages nothing: every key resolves to
/// `NotManaged`, so the disk catalogue is consulted.
#[derive(Debug, Default, Clone, Copy)]
pub struct NotManagedSkills;

#[async_trait]
impl ManagedSkillResolver for NotManagedSkills {
    async fn resolve_skill(
        &self,
        _owner: &UserId,
        _key: &str,
    ) -> Result<SkillResolution, ManagedSkillResolverError> {
        Ok(SkillResolution::NotManaged)
    }
}

/// A managed-skill authority with one scripted answer for every key.
#[derive(Debug, Clone)]
pub enum ScriptedSkills {
    Published(ResolvedManagedSkill),
    Withheld(WithheldReason),
    Unavailable,
}

#[async_trait]
impl ManagedSkillResolver for ScriptedSkills {
    async fn resolve_skill(
        &self,
        _owner: &UserId,
        key: &str,
    ) -> Result<SkillResolution, ManagedSkillResolverError> {
        match self {
            Self::Published(skill) => Ok(SkillResolution::Published(skill.clone())),
            Self::Withheld(reason) => Ok(SkillResolution::Withheld(*reason)),
            Self::Unavailable => Err(ManagedSkillResolverError::Unavailable(format!(
                "scripted outage resolving {key}"
            ))),
        }
    }
}

/// A tool-execution ledger with a fixed answer for every id.
#[derive(Debug, Clone, Copy)]
pub enum ToolExecutionLedger {
    Exists,
    Absent,
    Unavailable,
}

#[async_trait]
impl ToolExecutionLookup for ToolExecutionLedger {
    async fn execution_exists(&self, id: &McpExecutionId) -> Result<bool, RepositoryError> {
        match self {
            Self::Exists => Ok(true),
            Self::Absent => Ok(false),
            Self::Unavailable => Err(RepositoryError::database(format!(
                "ledger unavailable while checking {id}"
            ))),
        }
    }
}

pub fn not_managed_skills() -> DynManagedSkillResolver {
    Arc::new(NotManagedSkills)
}

pub fn scripted_skills(script: ScriptedSkills) -> DynManagedSkillResolver {
    Arc::new(script)
}

pub fn tool_execution_ledger(ledger: ToolExecutionLedger) -> DynToolExecutionLookup {
    Arc::new(ledger)
}

pub fn a2a_dependencies(pool: &DbPool) -> A2aDependencies {
    A2aDependencies {
        session_usage: crate::fixture_analytics_repositories(pool)
            .expect("analytics repositories")
            .sessions
            .owner(),
        instance_id: InstanceId::new("test-instance"),
        managed_skills: not_managed_skills(),
        tool_executions: tool_execution_ledger(ToolExecutionLedger::Exists),
    }
}

pub fn a2a_repositories(pool: &DbPool) -> A2ARepositories {
    A2ARepositories::new(pool, a2a_dependencies(pool)).expect("a2a repositories")
}
