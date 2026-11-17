//! The files capability's contribution points.
//!
//! - [`BlobHolderSlot`]: tables outside the document store that hold a document
//!   blob. Blob collection keeps every hash a holder still references; a holder
//!   name declared twice refuses the build.
//! - [`AgentAccessSlot`]: the one [`AgentAccessPolicy`] that decides agent
//!   caps. It is single-valued: more than one contributed policy is
//!   [`SlotError::MultipleEntries`]; none built yields
//!   [`DefaultAgentAccessPolicy`].
//! - [`DocumentsSlot`]: document codecs and template providers. The built
//!   [`DocumentSeams`] carries the per-format [`CodecRegistry`] and the
//!   template providers in composition order.
//!
//! Every builder method only stores; admission drops a disabled member's
//! builder before any factory runs. The `files` member opens all three
//! slots and contributes the core codecs, the core template provider and the
//! issue attachment holder, so each built value is present exactly when that
//! member is admitted. A factory that fails, or reads a value nothing
//! provides, refuses the build.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use systemprompt_extension::{Factory, Resolved, Slot, SlotEntry, SlotError, SlotOrder};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::documents::{DocumentCodec, DocumentTemplateProvider};

use crate::documents::agents::{AgentAccessPolicy, DefaultAgentAccessPolicy};
use crate::documents::codecs::CodecRegistry;
use crate::documents::repository::{BlobHolder, BlobHolders};

const BLOB_HOLDER_SLOT: &str = "files.blob_holder";
const AGENT_ACCESS_SLOT: &str = "files.agent_access";
const DOCUMENTS_SLOT: &str = "files.documents";

type Factories<T> = Vec<Box<dyn Factory<Arc<T>>>>;

/// Tables that hold a document blob outside the document store.
#[derive(Debug, Clone, Copy)]
pub struct BlobHolderSlot;

/// One member's blob holders, stored until the slot is built.
#[derive(Debug, Default)]
pub struct BlobHolderSlotBuilder {
    holders: Vec<BlobHolder>,
}

impl BlobHolderSlotBuilder {
    pub fn holder(&mut self, holder: BlobHolder) -> &mut Self {
        self.holders.push(holder);
        self
    }
}

impl Slot for BlobHolderSlot {
    type Builder = BlobHolderSlotBuilder;
    type Built = BlobHolders;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        _: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut seen = HashSet::new();
        let mut holders = Vec::new();
        for holder in entries.into_iter().flat_map(|entry| entry.builder.holders) {
            if !seen.insert(holder.name) {
                return Err(SlotError::DuplicateId {
                    slot: BLOB_HOLDER_SLOT,
                    id: holder.name.to_owned(),
                });
            }
            holders.push(holder);
        }
        Ok(BlobHolders::new(holders))
    }
}

/// The single agent access policy.
#[derive(Debug, Clone, Copy)]
pub struct AgentAccessSlot;

/// One member's agent access policy, stored until the slot is built.
#[derive(Default)]
pub struct AgentAccessSlotBuilder {
    policies: Factories<dyn AgentAccessPolicy>,
}

impl AgentAccessSlotBuilder {
    pub fn policy<F: Factory<Arc<dyn AgentAccessPolicy>>>(&mut self, f: F) -> &mut Self {
        self.policies.push(Box::new(f));
        self
    }
}

impl fmt::Debug for AgentAccessSlotBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentAccessSlotBuilder")
            .field("policies", &self.policies.len())
            .finish()
    }
}

/// The built agent access policy.
#[derive(Clone)]
pub struct AgentAccess {
    policy: Arc<dyn AgentAccessPolicy>,
}

impl AgentAccess {
    #[must_use]
    pub fn policy(&self) -> Arc<dyn AgentAccessPolicy> {
        Arc::clone(&self.policy)
    }
}

impl fmt::Debug for AgentAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentAccess")
            .field("policy", &self.policy.name())
            .finish()
    }
}

impl Slot for AgentAccessSlot {
    type Builder = AgentAccessSlotBuilder;
    type Built = AgentAccess;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let contributed: Vec<(ExtensionId, Box<dyn Factory<Arc<dyn AgentAccessPolicy>>>)> = entries
            .into_iter()
            .flat_map(|entry| {
                let owner = entry.owner;
                entry
                    .builder
                    .policies
                    .into_iter()
                    .map(move |factory| (owner.clone(), factory))
            })
            .collect();
        if contributed.len() > 1 {
            return Err(SlotError::MultipleEntries {
                slot: AGENT_ACCESS_SLOT,
                count: contributed.len(),
            });
        }
        let policy = match contributed.into_iter().next() {
            Some((owner, factory)) => factory
                .build(r)
                .map_err(|e| e.into_slot_error(AGENT_ACCESS_SLOT, &owner))?,
            None => Arc::new(DefaultAgentAccessPolicy),
        };
        Ok(AgentAccess { policy })
    }
}

/// Document codecs and template providers.
#[derive(Debug, Clone, Copy)]
pub struct DocumentsSlot;

/// One member's codecs and template providers, stored until the slot is
/// built.
#[derive(Default)]
pub struct DocumentsSlotBuilder {
    codecs: Factories<dyn DocumentCodec>,
    template_providers: Factories<dyn DocumentTemplateProvider>,
}

impl DocumentsSlotBuilder {
    pub fn codec<F: Factory<Arc<dyn DocumentCodec>>>(&mut self, f: F) -> &mut Self {
        self.codecs.push(Box::new(f));
        self
    }

    pub fn template_provider<F: Factory<Arc<dyn DocumentTemplateProvider>>>(
        &mut self,
        f: F,
    ) -> &mut Self {
        self.template_providers.push(Box::new(f));
        self
    }
}

impl fmt::Debug for DocumentsSlotBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentsSlotBuilder")
            .field("codecs", &self.codecs.len())
            .field("template_providers", &self.template_providers.len())
            .finish()
    }
}

/// The built document seams: the codec per format and the template
/// providers in composition order.
#[derive(Clone, Default)]
pub struct DocumentSeams {
    pub codecs: CodecRegistry,
    pub template_providers: Vec<Arc<dyn DocumentTemplateProvider>>,
}

impl fmt::Debug for DocumentSeams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentSeams")
            .field("codecs", &self.codecs)
            .field("template_providers", &self.template_providers.len())
            .finish()
    }
}

impl Slot for DocumentsSlot {
    type Builder = DocumentsSlotBuilder;
    type Built = DocumentSeams;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut codecs = Vec::new();
        let mut template_providers = Vec::new();
        for entry in entries {
            for factory in &entry.builder.codecs {
                codecs.push(
                    factory
                        .build(r)
                        .map_err(|e| e.into_slot_error(DOCUMENTS_SLOT, &entry.owner))?,
                );
            }
            for factory in &entry.builder.template_providers {
                template_providers.push(
                    factory
                        .build(r)
                        .map_err(|e| e.into_slot_error(DOCUMENTS_SLOT, &entry.owner))?,
                );
            }
        }
        Ok(DocumentSeams {
            codecs: CodecRegistry::from_codecs(codecs),
            template_providers,
        })
    }
}
