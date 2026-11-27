//! [`BrainEngine`]: what the brain adds to sessions while the instance
//! admits the extension.
//!
//! Four tools let the model read and question the team's memory and propose
//! to it: `brain_lookup`, `brain_why`, `brain_propose`, `brain_dispute`.
//! Confirming, approving, retracting, pinning and erasing are a person's
//! decisions and are never tools. The tools and the prompt section are
//! offered only while the instance has confirmed its brain is enabled; the
//! extension's generation follows that status, so the engine reads them again
//! when the probe settles after the instance was applied. The call hooks send
//! recall off when the person turned it off, and record the facts each
//! response says the gateway injected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod hooks;
mod operations;
mod render;
mod tools;

use std::sync::Arc;

use harness_engine::extension::{
    CallAugmenter, EngineExtension, ExtensionMetadata, ExtensionTool, GovernorObserver,
    PromptContext, PromptSection,
};
use sp_extension::ExtensionSettings;

use crate::extension::metadata;
use crate::service::BrainService;

const HEADING: &str = "Team memory";
const GUIDANCE: &str = "This organisation keeps a shared memory of confirmed facts. Facts it recalls for a request arrive in a <team_memory> block, each line tagged with its fact id; treat them as what the team has agreed, and name the fact id when an answer rests on one. Use brain_lookup to read facts by id, kind or subject, and brain_why to see why a fact ranks where it does. When the conversation settles something the team should remember, propose it with brain_propose; when a recalled fact is wrong or out of date, dispute it with brain_dispute and give the reason. A person confirms, retracts, pins and erases facts in the Brain screen: never say you did any of those.";

/// The engine half of the brain extension, built over the shared
/// [`BrainService`] and the person's brain settings.
#[derive(Debug, Clone)]
pub struct BrainEngine {
    service: BrainService,
    settings: ExtensionSettings,
}

impl BrainEngine {
    #[must_use]
    pub const fn new(service: BrainService, settings: ExtensionSettings) -> Self {
        Self { service, settings }
    }
}

impl EngineExtension for BrainEngine {
    fn metadata(&self) -> ExtensionMetadata {
        metadata()
    }

    fn generation(&self) -> u64 {
        u64::from(self.service.status().is_enabled())
    }

    fn tools(&self) -> Vec<ExtensionTool> {
        if self.service.status().is_enabled() {
            tools::all(&self.service)
        } else {
            Vec::new()
        }
    }

    fn prompt_sections(&self, _step: &PromptContext<'_>) -> Vec<PromptSection> {
        if !self.service.status().is_enabled() {
            return Vec::new();
        }
        vec![PromptSection {
            heading: HEADING.to_owned(),
            text: GUIDANCE.to_owned(),
        }]
    }

    fn observer(&self) -> Option<Arc<dyn GovernorObserver>> {
        Some(Arc::new(hooks::MemoryObserver::new(self.service.clone())))
    }

    fn augmenter(&self) -> Option<Arc<dyn CallAugmenter>> {
        Some(Arc::new(hooks::RecallSwitch::new(self.settings.clone())))
    }
}
