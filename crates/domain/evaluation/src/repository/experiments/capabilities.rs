//! Execution-only capabilities; these tokens are never user or worker
//! credentials. The principal an execution token authenticates to carries a
//! fixed `user` role rather than its owner's roles, so a run started by an
//! administrator cannot act as one; the owner id still scopes every read and
//! write the run performs.
//!
//! The session an execution token binds to is a `user_sessions` row owned by
//! the users domain; it is created and re-verified through
//! `AiSessionProvider`, never written here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ExecutionLease, lock_owner};
use crate::Result;
use crate::experiments::{conflict, missing};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::types::Json;
use systemprompt_identifiers::{EvalExecutionId, EvalWorkerId, SessionId, SessionSource, UserId};
use systemprompt_traits::{CreateAiSessionParams, DynAiSessionProvider};

pub const EXECUTION_TOKEN_PREFIX: &str = "spexec_";

#[derive(Serialize)]
pub struct ExecutionAccess {
    token: String,
    pub session_id: SessionId,
}

impl std::fmt::Debug for ExecutionAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionAccess")
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

impl ExecutionAccess {
    pub fn expose_token(&self) -> &str {
        &self.token
    }
}

#[derive(Debug, Deserialize)]
pub struct ExecutionIdentity {
    pub owner_id: UserId,
    pub execution_id: EvalExecutionId,
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExecutionPrincipal {
    pub identity: ExecutionIdentity,
    pub session_id: SessionId,
}

#[derive(Clone)]
pub struct ExecutionCapabilityRepository {
    pool: PgPool,
    sessions: DynAiSessionProvider,
}

impl std::fmt::Debug for ExecutionCapabilityRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionCapabilityRepository")
            .finish_non_exhaustive()
    }
}

impl ExecutionCapabilityRepository {
    pub const fn new(pool: PgPool, sessions: DynAiSessionProvider) -> Self {
        Self { pool, sessions }
    }

    pub async fn issue(&self, owner: &UserId, lease: &ExecutionLease) -> Result<ExecutionAccess> {
        let mut tx = self.pool.begin().await?;
        lock_owner(&mut tx, owner).await?;
        let eligible = sqlx::query_scalar!(
            "SELECT x.id FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id JOIN eval_workers w ON w.id=x.lease_owner WHERE x.id=$1 AND e.owner_id=$2 AND w.owner_id=$2 AND w.id=$3 AND w.enabled AND w.expires_at>NOW() AND x.fencing_token=$4 AND x.status='running' AND e.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW()",
            lease.execution_id.as_str(), owner.as_str(), lease.worker_id.as_str(), lease.fencing_token
        ).fetch_optional(&mut *tx).await?;
        if eligible.is_none() {
            return Err(conflict("Execution is not eligible for credentials"));
        }
        let existing = sqlx::query_scalar!("SELECT session_id FROM eval_session_bindings WHERE execution_id=$1 AND fencing_token=$2 LIMIT 1",
            lease.execution_id.as_str(), lease.fencing_token).fetch_optional(&mut *tx).await?;
        let session_id = if let Some(id) = existing {
            SessionId::new(id)
        } else {
            let id = SessionId::generate();
            self.sessions
                .create_session(CreateAiSessionParams {
                    session_id: &id,
                    user_id: Some(owner),
                    session_source: SessionSource::Api,
                    expires_at: chrono::Utc::now() + chrono::Duration::minutes(31),
                })
                .await?;
            sqlx::query!("INSERT INTO eval_session_bindings(session_id,execution_id,owner_id,fencing_token) VALUES($1,$2,$3,$4)",
                id.as_str(), lease.execution_id.as_str(), owner.as_str(), lease.fencing_token).execute(&mut *tx).await?;
            id
        };
        let token = format!(
            "{EXECUTION_TOKEN_PREFIX}{}.{}",
            EvalWorkerId::generate(),
            EvalWorkerId::generate()
        );
        let digest = hex::encode(Sha256::digest(token.as_bytes()));
        sqlx::query!("UPDATE eval_execution_capabilities SET revoked_at=NOW() WHERE execution_id=$1 AND revoked_at IS NULL",
            lease.execution_id.as_str()).execute(&mut *tx).await?;
        sqlx::query!("INSERT INTO eval_execution_capabilities(token_hash,execution_id,worker_id,session_id,fencing_token,expires_at) SELECT $1,$2,$3,$4,$5,LEAST(x.deadline_at,NOW()+INTERVAL '31 minutes') FROM eval_executions x WHERE x.id=$2",
            digest,lease.execution_id.as_str(),lease.worker_id.as_str(),session_id.as_str(),lease.fencing_token).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(ExecutionAccess { token, session_id })
    }

    pub async fn authenticate(&self, token: &str, environment: &str) -> Result<ExecutionPrincipal> {
        if !token.starts_with(EXECUTION_TOKEN_PREFIX) || token.len() > 128 {
            return Err(missing("Execution credential unavailable"));
        }
        let digest = hex::encode(Sha256::digest(token.as_bytes()));
        let principal = sqlx::query_scalar!(
            r#"SELECT jsonb_build_object('identity',jsonb_build_object('owner_id',e.owner_id,'execution_id',x.id,'roles',jsonb_build_array('user')),'session_id',c.session_id) AS "principal!: Json<ExecutionPrincipal>" FROM eval_execution_capabilities c JOIN eval_executions x ON x.id=c.execution_id JOIN eval_experiments e ON e.id=x.experiment_id JOIN eval_workers w ON w.id=c.worker_id WHERE c.token_hash=$1 AND w.environment=$2 AND w.owner_id=e.owner_id AND w.enabled AND w.expires_at>NOW() AND c.revoked_at IS NULL AND c.expires_at>NOW() AND x.lease_owner=c.worker_id AND x.fencing_token=c.fencing_token AND x.status='running' AND e.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW()"#,
            digest,environment
        ).fetch_optional(&self.pool).await?.ok_or_else(|| missing("Execution credential unavailable"))?.0;
        let session = self
            .sessions
            .find_live_session(&principal.session_id)
            .await?;
        match session {
            Some(live) if live.user_id.as_ref() == Some(&principal.identity.owner_id) => {
                Ok(principal)
            },
            _ => Err(missing("Execution credential unavailable")),
        }
    }
}

#[derive(Debug)]
pub struct ExecutionIdentityBuilder {
    owner_id: UserId,
    execution_id: EvalExecutionId,
    roles: Vec<String>,
}

impl ExecutionIdentity {
    pub const fn builder(
        owner_id: UserId,
        execution_id: EvalExecutionId,
    ) -> ExecutionIdentityBuilder {
        ExecutionIdentityBuilder {
            owner_id,
            execution_id,
            roles: Vec::new(),
        }
    }
}

impl ExecutionIdentityBuilder {
    pub fn roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }
    pub fn build(self) -> ExecutionIdentity {
        ExecutionIdentity {
            owner_id: self.owner_id,
            execution_id: self.execution_id,
            roles: self.roles,
        }
    }
}
