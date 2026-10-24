//! The `gateway_policies` config plane: `services/gateway/policies.yaml`
//! projected into `ai_gateway_policies`.
//!
//! Each policy is one plane entity keyed by its name; the value is the
//! canonical JSON of its `enabled`, `priority` and spec. The spec is decoded
//! into [`super::GatewayPolicySpec`] and re-encoded with every object's keys
//! sorted, so a file and a `JSONB` row (which reorders keys) compare equal
//! exactly when they mean the same policy; a row whose spec no longer decodes
//! keeps its raw text and shows as drift. The request path reads the rows on
//! every call, so an apply takes effect without a restart. Boot never writes
//! this plane: `config apply` and `/admin/sync` do.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use systemprompt_ai::repository::{AiGatewayPolicyRepository, GatewayPolicyWrite};
use systemprompt_database::PlaneTransaction;
use systemprompt_extension::prelude::{
    ApplyOutcome, ConfigPlane, ConfigPlaneError, ConfigPlaneResult, DeclaredState, PlaneContext,
    PlaneSnapshot, PlaneTx, ResolvedEntities,
};

use super::config::{GatewayPolicyConfig, GatewayPolicyEntry};
use super::spec::GatewayPolicySpec;

pub const GATEWAY_POLICIES_FILE: &str = "gateway/policies.yaml";
pub const GATEWAY_POLICIES_PLANE: &str = "gateway_policies";

#[derive(Debug, Serialize, Deserialize)]
struct PolicyValue {
    enabled: bool,
    priority: i32,
    // JSON: the policy spec as canonical JSON, keys sorted at every level.
    spec: Value,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GatewayPoliciesPlane;

fn invalid(error: &dyn std::fmt::Display) -> ConfigPlaneError {
    ConfigPlaneError::Invalid {
        plane: GATEWAY_POLICIES_PLANE.to_owned(),
        path: GATEWAY_POLICIES_FILE.to_owned(),
        message: error.to_string(),
    }
}

fn backend(error: impl std::error::Error + Send + Sync + 'static) -> ConfigPlaneError {
    ConfigPlaneError::Backend {
        plane: GATEWAY_POLICIES_PLANE.to_owned(),
        source: Box::new(error),
    }
}

// JSON: re-orders every object of a policy spec so equal specs print equally.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k, sorted(v)))
                    .collect::<Map<_, _>>(),
            )
        },
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn encode(enabled: bool, priority: i32, spec: &GatewayPolicySpec) -> ConfigPlaneResult<String> {
    let spec = sorted(serde_json::to_value(spec).map_err(backend)?);
    serde_json::to_string(&PolicyValue {
        enabled,
        priority,
        spec,
    })
    .map_err(backend)
}

fn entry_value(entry: &GatewayPolicyEntry) -> ConfigPlaneResult<String> {
    encode(entry.enabled, entry.priority, &entry.spec)
}

#[async_trait]
impl ConfigPlane for GatewayPoliciesPlane {
    fn name(&self) -> &'static str {
        GATEWAY_POLICIES_PLANE
    }

    fn declared_path(&self) -> &'static str {
        GATEWAY_POLICIES_FILE
    }

    fn supports_scoped_apply(&self) -> bool {
        true
    }

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        let path = services_root.join(GATEWAY_POLICIES_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(ConfigPlaneError::Unreadable {
                    plane: GATEWAY_POLICIES_PLANE.to_owned(),
                    path: GATEWAY_POLICIES_FILE.to_owned(),
                    source: Box::new(error),
                });
            },
        };
        let config: GatewayPolicyConfig = serde_yaml::from_str(&text).map_err(|e| invalid(&e))?;
        config.validate().map_err(|e| invalid(&e))?;
        let entries = config
            .policies
            .iter()
            .map(|entry| Ok((entry.name.clone(), entry_value(entry)?)))
            .collect::<ConfigPlaneResult<PlaneSnapshot>>()?;
        Ok(Some(DeclaredState::from_entries(entries)))
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        let tx = tx.get::<PlaneTransaction>()?;
        let rows = AiGatewayPolicyRepository::list_all_on(tx)
            .await
            .map_err(backend)?;
        rows.into_iter()
            .map(|row| {
                let value = serde_json::from_value::<GatewayPolicySpec>(row.spec.clone())
                    .map_or_else(
                        |_| Ok(row.spec.to_string()),
                        |spec| encode(row.enabled, row.priority, &spec),
                    )?;
                Ok((row.name, value))
            })
            .collect()
    }

    async fn write(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
        entities: &ResolvedEntities,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        let tx = tx.get::<PlaneTransaction>()?;
        let mut outcome = ApplyOutcome::default();
        for (name, resolved) in entities {
            if let Some(value) = &resolved.value {
                let policy: PolicyValue = serde_json::from_str(value).map_err(backend)?;
                let source = resolved.source.to_string();
                let row = GatewayPolicyWrite {
                    name,
                    spec: &policy.spec,
                    enabled: policy.enabled,
                    priority: policy.priority,
                    source: &source,
                };
                AiGatewayPolicyRepository::upsert_on(tx, &row)
                    .await
                    .map_err(backend)?;
                outcome.updated += 1;
            } else {
                AiGatewayPolicyRepository::delete_on(tx, name)
                    .await
                    .map_err(backend)?;
                outcome.deleted += 1;
            }
        }
        Ok(outcome)
    }

    async fn export(
        &self,
        ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<Option<String>> {
        let current = self.read_current(ctx, tx).await?;
        if current.is_empty() {
            return Ok(None);
        }
        self.render_declarations(&current).map(Some)
    }

    fn render_declarations(&self, entities: &PlaneSnapshot) -> ConfigPlaneResult<String> {
        let policies = entities
            .iter()
            .map(|(name, value)| {
                let policy: PolicyValue = serde_json::from_str(value).map_err(backend)?;
                Ok(GatewayPolicyEntry {
                    name: name.clone(),
                    enabled: policy.enabled,
                    priority: policy.priority,
                    spec: serde_json::from_value(policy.spec).map_err(backend)?,
                })
            })
            .collect::<ConfigPlaneResult<Vec<_>>>()?;
        serde_yaml::to_string(&GatewayPolicyConfig { policies }).map_err(backend)
    }
}
