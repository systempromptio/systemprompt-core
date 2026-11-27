//! What the admitted extensions contribute under one enablement, one Governor
//! generation and one generation of each admitted extension: read once, shared
//! by every step until any of them changes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::config::{AgentDef, Command, Paths};

use crate::run::extension::{
    CallAugmenter, Contributed, EnabledExtensions, ExtensionSet, ExtensionTool, GovernorObserver,
    ToolReviewer,
};
use crate::run::governor::{Governor, McpSpec};

pub(crate) struct Contributions {
    pub(crate) revision: u64,
    pub(super) generation: u64,
    pub(super) enabled: Option<EnabledExtensions>,
    pub(super) extension_generations: Vec<u64>,
    pub(crate) tools: Vec<Contributed<ExtensionTool>>,
    pub(crate) agents: Vec<Contributed<AgentDef>>,
    pub(crate) commands: Vec<Contributed<Command>>,
    pub(crate) mcp_servers: Vec<McpSpec>,
    pub(super) observers: Vec<Contributed<Arc<dyn GovernorObserver>>>,
    pub(super) reviewers: Vec<Contributed<Arc<dyn ToolReviewer>>>,
    pub(super) augmenters: Vec<Contributed<Arc<dyn CallAugmenter>>>,
}

pub(super) struct Reading<'a> {
    pub(super) extensions: &'a ExtensionSet,
    pub(super) governor: &'a dyn Governor,
    pub(super) paths: &'a Paths,
    pub(super) generation: u64,
    pub(super) enabled: Option<EnabledExtensions>,
    pub(super) extension_generations: Vec<u64>,
    pub(super) revision: u64,
}

impl Contributions {
    pub(super) fn read(reading: Reading<'_>) -> Self {
        let Reading {
            extensions,
            governor,
            paths,
            generation,
            enabled,
            extension_generations,
            revision,
        } = reading;
        let admitted = enabled.as_ref();
        let mcp_servers = merge_mcp(governor.mcp_servers(), extensions.mcp_servers(admitted));
        Self {
            revision,
            generation,
            tools: extensions.tools(admitted),
            agents: extensions.agents(admitted, paths),
            commands: extensions.commands(admitted),
            mcp_servers,
            observers: extensions.observers(admitted),
            reviewers: extensions.reviewers(admitted),
            augmenters: extensions.augmenters(admitted),
            enabled,
            extension_generations,
        }
    }

    pub(super) fn is_for(
        &self,
        generation: u64,
        enabled: Option<&EnabledExtensions>,
        extension_generations: &[u64],
    ) -> bool {
        self.generation == generation
            && self.enabled.as_ref() == enabled
            && self.extension_generations == extension_generations
    }
}

fn merge_mcp(managed: Vec<McpSpec>, contributed: Vec<Contributed<McpSpec>>) -> Vec<McpSpec> {
    let mut servers = managed;
    for Contributed { extension, item } in contributed {
        if servers.iter().any(|server| server.name == item.name) {
            tracing::warn!(
                extension = %extension,
                server = %item.name,
                "an extension MCP server shares the name of one already offered; not started"
            );
            continue;
        }
        servers.push(item);
    }
    servers
}
