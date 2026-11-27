//! The engine's resolved extensions, and every contribution read from the ones
//! an instance's enablement admits, each kept with its extension's id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use harness_config::{AgentDef, Command, Paths};

use super::contribution::{Contributed, ExtensionTool, PromptContext, PromptSection};
use super::enabled::EnabledExtensions;
use super::engine::EngineExtension;
use super::error::ResolveError;
use super::hooks::{CallAugmenter, GovernorObserver, ToolReviewer};
use super::resolve::{Resolved, admitted, resolve};
use crate::governor::McpSpec;

type Entry = Resolved<Arc<dyn EngineExtension>>;

/// Every linked engine extension, resolved: one per id, highest priority
/// first, dependencies present and acyclic.
///
/// Each reader takes the instance's enablement: required extensions always
/// contribute, any other only when `enabled` lists it and every extension it
/// depends on is admitted too. `None` admits the required ones alone.
#[derive(Clone, Default)]
pub struct ExtensionSet {
    entries: Vec<Entry>,
}

impl ExtensionSet {
    pub fn resolve(
        extensions: impl IntoIterator<Item = Arc<dyn EngineExtension>>,
    ) -> Result<Self, ResolveError> {
        let candidates = extensions.into_iter().map(|value| Resolved {
            metadata: value.metadata(),
            value,
        });
        Ok(Self {
            entries: resolve(candidates)?,
        })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn admitted(&self, enabled: Option<&EnabledExtensions>) -> Vec<&Entry> {
        admitted(&self.entries, enabled)
    }

    pub fn tools(&self, enabled: Option<&EnabledExtensions>) -> Vec<Contributed<ExtensionTool>> {
        self.collect(enabled, |extension| extension.tools())
    }

    pub fn prompt_sections(
        &self,
        enabled: Option<&EnabledExtensions>,
        step: &PromptContext<'_>,
    ) -> Vec<Contributed<PromptSection>> {
        self.collect(enabled, |extension| extension.prompt_sections(step))
    }

    pub fn agents(
        &self,
        enabled: Option<&EnabledExtensions>,
        paths: &Paths,
    ) -> Vec<Contributed<AgentDef>> {
        self.collect(enabled, |extension| extension.agents(paths))
    }

    pub fn commands(&self, enabled: Option<&EnabledExtensions>) -> Vec<Contributed<Command>> {
        self.collect(enabled, |extension| extension.commands())
    }

    pub fn mcp_servers(&self, enabled: Option<&EnabledExtensions>) -> Vec<Contributed<McpSpec>> {
        self.collect(enabled, |extension| extension.mcp_servers())
    }

    pub fn observers(
        &self,
        enabled: Option<&EnabledExtensions>,
    ) -> Vec<Contributed<Arc<dyn GovernorObserver>>> {
        self.collect(enabled, |extension| extension.observer())
    }

    pub fn reviewers(
        &self,
        enabled: Option<&EnabledExtensions>,
    ) -> Vec<Contributed<Arc<dyn ToolReviewer>>> {
        self.collect(enabled, |extension| extension.reviewer())
    }

    pub fn augmenters(
        &self,
        enabled: Option<&EnabledExtensions>,
    ) -> Vec<Contributed<Arc<dyn CallAugmenter>>> {
        self.collect(enabled, |extension| extension.augmenter())
    }

    fn collect<T, I>(
        &self,
        enabled: Option<&EnabledExtensions>,
        read: impl Fn(&dyn EngineExtension) -> I,
    ) -> Vec<Contributed<T>>
    where
        I: IntoIterator<Item = T>,
    {
        self.admitted(enabled)
            .into_iter()
            .flat_map(|entry| {
                read(entry.value.as_ref())
                    .into_iter()
                    .map(|item| Contributed {
                        extension: entry.metadata.id.clone(),
                        item,
                    })
            })
            .collect()
    }
}

impl fmt::Debug for ExtensionSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.entries.iter().map(|entry| &entry.metadata))
            .finish()
    }
}
