//! General baseline capture preserves immutable candidates and reconciles
//! incoming authoring changes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::captures::IncomingRevision;
use super::catalog::invalid;
use super::{BaselineScope, InventoryEntry, InventoryService};
use crate::managed::{
    AssetDigest, AssetFile, ManagedError, NewResource, NewRevision, ResourceKind, Result,
    RevisionFiles, SnapshotProvenance, SourceSpec, capture_inventory_files,
};
use std::collections::BTreeMap;
use std::path::Path;
use systemprompt_identifiers::{ManagedReconciliationId, ResourceRevisionId, UserId};
use systemprompt_models::feedback::inventory::InventoryAvailability;
use systemprompt_models::services::ServicesConfig;

struct AuthoringCapture {
    files: RevisionFiles,
    previous: Option<ResourceRevisionId>,
}

impl InventoryService {
    pub(super) async fn capture_entry(
        &self,
        scope: &BaselineScope<'_>,
        entry: &InventoryEntry,
    ) -> Result<(ResourceRevisionId, Option<ManagedReconciliationId>)> {
        if entry.availability != InventoryAvailability::Available {
            return Err(invalid(
                "Inventory entry is unavailable, withdrawn or conflicting; resolve its diagnostic before baseline capture",
            ));
        }
        let owner = scope.owner;
        let source_spec = if let Some(source) = &entry.source_id {
            Some(self.repository.get_source(owner, source).await?)
        } else {
            None
        };
        if entry.configured_key.is_none()
            || matches!(
                source_spec,
                Some(SourceSpec::Git { .. } | SourceSpec::Managed)
            )
        {
            return self.retained_baseline(owner, entry).await;
        }
        let canonical = std::fs::canonicalize(scope.root)?;
        if let Some(SourceSpec::LocalTree { root: bound }) = source_spec
            && std::fs::canonicalize(bound)? != canonical
        {
            return Err(invalid(
                "Configured source binding does not match the active services root",
            ));
        }
        let files = configured_files(&canonical, entry, scope.services)?;
        let previous = self
            .repository
            .inventory_authoring_head(owner, &entry.entry_id)
            .await?;
        if let Some(revision) = previous.as_ref().or(entry.latest_revision_id.as_ref()) {
            let retained = self.repository.get_revision_files(owner, revision).await?;
            if same_files(&retained, &files) {
                self.repository
                    .advance_inventory_head(owner, &entry.entry_id, previous.as_ref(), revision)
                    .await?;
                return Ok((
                    entry
                        .published_revision_id
                        .clone()
                        .unwrap_or_else(|| revision.clone()),
                    None,
                ));
            }
            if previous.is_none() {
                return Err(invalid(
                    "Existing candidate differs from authoring files and has no retained upstream base; explicit reconciliation is required",
                ));
            }
        }
        self.capture_incoming(
            scope,
            entry,
            &canonical,
            AuthoringCapture { files, previous },
        )
        .await
    }

    async fn retained_baseline(
        &self,
        owner: &UserId,
        entry: &InventoryEntry,
    ) -> Result<(ResourceRevisionId, Option<ManagedReconciliationId>)> {
        let revision = entry
            .published_revision_id
            .as_ref()
            .or(entry.latest_revision_id.as_ref())
            .ok_or_else(|| invalid("No retained revision is available for baseline"))?;
        self.repository.get_revision_bundle(owner, revision).await?;
        Ok((revision.clone(), None))
    }

    async fn capture_incoming(
        &self,
        scope: &BaselineScope<'_>,
        entry: &InventoryEntry,
        canonical: &Path,
        capture: AuthoringCapture,
    ) -> Result<(ResourceRevisionId, Option<ManagedReconciliationId>)> {
        let AuthoringCapture { files, previous } = capture;
        let owner = scope.owner;
        let (source, resource) = self
            .bind_authoring_resource(owner, scope.actor, entry, canonical)
            .await?;
        let snapshot = self
            .repository
            .capture_snapshot(
                owner,
                &source,
                &SnapshotProvenance {
                    source_kind: "local_tree".to_owned(),
                    commit: None,
                    tree_digest: AssetDigest::of(&serde_jcs::to_vec(&files)?),
                    importer_version: "inventory-capture-v1".to_owned(),
                },
            )
            .await?;
        let dependencies = if let Some(base) = &previous {
            self.repository
                .get_revision(owner, base)
                .await?
                .dependencies
        } else {
            BTreeMap::new()
        };
        let incoming = self
            .repository
            .create_revision(
                owner,
                &NewRevision {
                    resource_id: resource.clone(),
                    snapshot_id: snapshot,
                    parent_id: previous.clone(),
                    files,
                    dependencies,
                    rationale: "Inventory authoring capture; publication selection unchanged"
                        .to_owned(),
                },
            )
            .await?;
        let reconciliation = self
            .repository
            .reconcile_inventory_incoming(
                owner,
                &IncomingRevision {
                    resource: &resource,
                    base: previous.as_ref(),
                    candidate: entry.latest_revision_id.as_ref(),
                    incoming: &incoming,
                },
            )
            .await?;
        self.repository
            .advance_inventory_head(owner, &entry.entry_id, previous.as_ref(), &incoming)
            .await?;
        Ok((incoming, reconciliation))
    }
    async fn bind_authoring_resource(
        &self,
        owner: &UserId,
        actor: &UserId,
        entry: &InventoryEntry,
        canonical: &Path,
    ) -> Result<(
        systemprompt_identifiers::ManagedSourceId,
        systemprompt_identifiers::ManagedResourceId,
    )> {
        let source = if let Some(source) = &entry.source_id {
            source.clone()
        } else {
            let path = canonical
                .to_str()
                .ok_or_else(|| invalid("Services root must be UTF-8"))?;
            let name = format!(
                "inventory-authoring-{}",
                &AssetDigest::of(path.as_bytes()).as_str()[..16]
            );
            self.repository
                .register_source(
                    owner,
                    &name,
                    &SourceSpec::LocalTree {
                        root: path.to_owned(),
                    },
                )
                .await?
        };
        let kind = match entry.kind.as_str() {
            "skill" => ResourceKind::Skill,
            "plugin" => ResourceKind::Plugin,
            "marketplace" => ResourceKind::Marketplace,
            _ => ResourceKind::Supporting,
        };
        let resource = if let Some(resource) = &entry.resource_id {
            resource.clone()
        } else {
            let resource_key = if matches!(kind, ResourceKind::Supporting) {
                format!("{}_{}", entry.kind, entry.resource_key)
            } else {
                entry.resource_key.clone()
            };
            let resource = self
                .repository
                .bind_resource(
                    owner,
                    &NewResource {
                        source_id: source.clone(),
                        upstream_key: entry
                            .configured_key
                            .clone()
                            .ok_or_else(|| invalid("Missing configured path"))?,
                        kind,
                        resource_key,
                    },
                )
                .await?;
            self.repository
                .bind_inventory_resource(owner, actor, &entry.entry_id, &resource)
                .await?;
            resource
        };
        Ok((source, resource))
    }
}

pub(super) fn configured_files(
    root: &Path,
    entry: &InventoryEntry,
    services: &ServicesConfig,
) -> Result<RevisionFiles> {
    let inline = match entry.kind.as_str() {
        "agent" => Some(
            serde_yaml::to_string(
                services
                    .agents
                    .get(&entry.resource_key)
                    .ok_or_else(|| invalid("Configured agent disappeared"))?,
            )
            .map_err(|error| {
                ManagedError::Invalid(format!("Agent configuration cannot be captured: {error}"))
            })?,
        ),
        "mcp" => Some(
            serde_yaml::to_string(
                services
                    .mcp_servers
                    .get(&entry.resource_key)
                    .ok_or_else(|| invalid("Configured MCP server disappeared"))?,
            )
            .map_err(|error| {
                ManagedError::Invalid(format!("MCP configuration cannot be captured: {error}"))
            })?,
        ),
        _ => None,
    };
    if let Some(config) = inline {
        let files = RevisionFiles(BTreeMap::from([(
            "config.yaml".to_owned(),
            AssetFile {
                bytes: config.into_bytes(),
                media_type: "application/yaml".to_owned(),
                executable: false,
            },
        )]));
        files.validate()?;
        Ok(files)
    } else {
        capture_inventory_files(
            root,
            entry
                .configured_key
                .as_deref()
                .ok_or_else(|| invalid("Missing authoring path"))?,
        )
    }
}

fn same_files(a: &RevisionFiles, b: &RevisionFiles) -> bool {
    a.0.len() == b.0.len()
        && a.0.iter().all(|(path, file)| {
            b.0.get(path).is_some_and(|other| {
                file.bytes == other.bytes && file.executable == other.executable
            })
        })
}
