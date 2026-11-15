//! [`ToolsetSlot`]: the toolset host's contribution point.
//!
//! A member contributes a toolset with
//! `c.slot::<ToolsetSlot>().toolset(id, build)`: `id` is the YAML id a
//! deployment lists under an MCP server's `toolsets:`, and `build` makes the
//! toolset from the server's [`HostContext`]. The builder stores and runs
//! nothing. Admission drops a disabled member's builder, and the kernel then
//! builds the slot once and inserts the result as `Arc<Toolsets>`. The
//! [`McpHostExtension`](crate::McpHostExtension) opens the slot with core's
//! `issues` and `documents` toolsets, so the built value is present whenever
//! that member is admitted.
//!
//! The build refuses two toolsets with one id ([`SlotError::DuplicateId`]),
//! and refuses a composition in which an enabled internal MCP server that
//! names no `binary:` lists a toolset id no admitted member contributes
//! ([`UnregisteredToolsets`]): such a server would be hosted by this binary
//! and could not start. An empty set of toolsets is checked like any other,
//! never read as "nothing to check".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fmt;

use systemprompt_extension::{MissingDependency, Resolved, Slot, SlotEntry, SlotError, SlotOrder, StandingTag};
use systemprompt_identifiers::{ExtensionId, McpServerId};
use systemprompt_manifest::ServicesConfig;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::mcp::McpServerType;

use crate::HostContext;
use crate::toolsets::ToolsetBuildResult;

const SLOT: &str = "mcp_toolsets";

pub type ToolsetBuildFn = fn(&HostContext, &McpServerId) -> ToolsetBuildResult;

/// The toolset contribution point, owned by the `mcp_host` capability.
#[derive(Debug, Clone, Copy)]
pub struct ToolsetSlot;

/// One member's toolsets, stored until the slot is built.
#[derive(Debug, Default)]
pub struct ToolsetSlotBuilder {
    toolsets: Vec<(&'static str, ToolsetBuildFn)>,
}

impl ToolsetSlotBuilder {
    pub fn toolset(&mut self, id: &'static str, build: ToolsetBuildFn) -> &mut Self {
        self.toolsets.push((id, build));
        self
    }
}

/// Every admitted toolset, keyed by its YAML id.
#[derive(Clone, Default)]
pub struct Toolsets {
    factories: BTreeMap<&'static str, ToolsetBuildFn>,
}

impl Toolsets {
    #[must_use]
    pub fn find(&self, id: &str) -> Option<ToolsetBuildFn> {
        self.factories.get(id).copied()
    }

    #[must_use]
    pub fn ids(&self) -> Vec<&'static str> {
        self.factories.keys().copied().collect()
    }
}

impl fmt::Debug for Toolsets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Toolsets")
            .field("ids", &self.ids())
            .finish()
    }
}

/// One toolset id an enabled, binary-less internal server lists that no
/// admitted member contributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnregisteredToolset {
    pub server: String,
    pub toolset: String,
}

/// The hosted toolset ids no admitted member contributes.
#[derive(Debug, thiserror::Error)]
#[error(
    "MCP servers list toolsets no composed member contributes: {} (contributed: {})",
    describe(.missing),
    describe_ids(.registered)
)]
pub struct UnregisteredToolsets {
    pub missing: Vec<UnregisteredToolset>,
    pub registered: Vec<&'static str>,
}

fn describe(missing: &[UnregisteredToolset]) -> String {
    missing
        .iter()
        .map(|entry| format!("mcp_servers.{}.toolsets: '{}'", entry.server, entry.toolset))
        .collect::<Vec<_>>()
        .join(", ")
}

fn describe_ids(registered: &[&'static str]) -> String {
    if registered.is_empty() {
        "none".to_owned()
    } else {
        registered.join(", ")
    }
}

#[must_use]
pub fn unregistered_toolsets(
    services: &ServicesConfig,
    known: &[&str],
) -> Vec<UnregisteredToolset> {
    let mut names: Vec<&String> = services.mcp_servers.keys().collect();
    names.sort();
    names
        .into_iter()
        .filter_map(|name| {
            let deployment = &services.mcp_servers[name];
            let hosted = deployment.enabled
                && matches!(deployment.server_type, McpServerType::Internal)
                && deployment.binary.is_none();
            hosted.then_some((name, deployment))
        })
        .flat_map(|(name, deployment)| {
            deployment
                .enabled_toolsets()
                .filter(|toolset| !known.contains(&toolset.id.as_str()))
                .map(|toolset| UnregisteredToolset {
                    server: name.clone(),
                    toolset: toolset.id.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

impl Slot for ToolsetSlot {
    type Builder = ToolsetSlotBuilder;
    type Built = Toolsets;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let services = r.get::<ServicesView>()?;
        let owner = slot_owner(&entries);
        let mut factories: BTreeMap<&'static str, ToolsetBuildFn> = BTreeMap::new();
        for entry in entries {
            for (id, build) in entry.builder.toolsets {
                if factories.insert(id, build).is_some() {
                    return Err(SlotError::DuplicateId {
                        slot: SLOT,
                        id: id.to_owned(),
                    });
                }
            }
        }
        let built = Toolsets { factories };
        let registered = built.ids();
        let missing = unregistered_toolsets(&services.config(), &registered);
        if missing.is_empty() {
            return Ok(built);
        }
        let source = Box::new(UnregisteredToolsets {
            missing,
            registered,
        });
        match owner {
            Some(owner) => Err(SlotError::Entry {
                slot: SLOT,
                owner,
                source,
            }),
            None => Err(SlotError::Missing(MissingDependency {
                ty: "mcp_host::contract::ToolsetSlot entries",
            })),
        }
    }
}

fn slot_owner(entries: &[SlotEntry<ToolsetSlotBuilder>]) -> Option<ExtensionId> {
    entries
        .iter()
        .find(|entry| matches!(entry.standing, StandingTag::Core))
        .or_else(|| entries.first())
        .map(|entry| entry.owner.clone())
}
