//! `services/ai/models.yaml`: what a model is, stated once.
//!
//! A model is its identity, the name the upstream knows it by
//! (`upstream_model`, defaulting to the id), its capabilities, limits, base
//! price, optional data-governance override and the client families it is
//! advertised to. `capabilities` lists only what differs from the defaults of
//! the provider that serves it, so the ten-flag block is not repeated on every
//! model. `clients: []` is "not advertised": the model still dispatches when a
//! caller names it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ModelId;
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::ModelLimits;

use crate::services::ai::{ModelCapabilities, ModelGovernance, ModelPricing};

/// One capability flag a model can override on its provider's defaults.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Vision,
    AudioInput,
    VideoInput,
    ImageGeneration,
    AudioGeneration,
    Streaming,
    Tools,
    StructuredOutput,
    SystemPrompts,
    ImageResolutionConfig,
    Reasoning,
    PromptCaching,
}

impl Capability {
    pub const ALL: [Self; 12] = [
        Self::Vision,
        Self::AudioInput,
        Self::VideoInput,
        Self::ImageGeneration,
        Self::AudioGeneration,
        Self::Streaming,
        Self::Tools,
        Self::StructuredOutput,
        Self::SystemPrompts,
        Self::ImageResolutionConfig,
        Self::Reasoning,
        Self::PromptCaching,
    ];

    const fn flag(self, capabilities: &mut ModelCapabilities) -> &mut bool {
        match self {
            Self::Vision => &mut capabilities.vision,
            Self::AudioInput => &mut capabilities.audio_input,
            Self::VideoInput => &mut capabilities.video_input,
            Self::ImageGeneration => &mut capabilities.image_generation,
            Self::AudioGeneration => &mut capabilities.audio_generation,
            Self::Streaming => &mut capabilities.streaming,
            Self::Tools => &mut capabilities.tools,
            Self::StructuredOutput => &mut capabilities.structured_output,
            Self::SystemPrompts => &mut capabilities.system_prompts,
            Self::ImageResolutionConfig => &mut capabilities.image_resolution_config,
            Self::Reasoning => &mut capabilities.reasoning,
            Self::PromptCaching => &mut capabilities.prompt_caching,
        }
    }
}

#[must_use]
pub fn capability_flags(capabilities: ModelCapabilities) -> BTreeMap<Capability, bool> {
    let mut source = capabilities;
    Capability::ALL
        .into_iter()
        .map(|capability| (capability, *capability.flag(&mut source)))
        .collect()
}

#[must_use]
pub fn apply_capability_overrides(
    defaults: ModelCapabilities,
    overrides: &BTreeMap<Capability, bool>,
) -> ModelCapabilities {
    let mut resolved = defaults;
    for (capability, value) in overrides {
        *capability.flag(&mut resolved) = *value;
    }
    resolved
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capabilities: BTreeMap<Capability, bool>,
    #[serde(default)]
    pub limits: ModelLimits,
    #[serde(default)]
    pub pricing: ModelPricing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance: Option<ModelGovernance>,
    pub clients: Vec<ClientFamily>,
}

impl ModelEntry {
    #[must_use]
    pub fn new(clients: Vec<ClientFamily>) -> Self {
        Self {
            upstream_model: None,
            display_name: None,
            capabilities: BTreeMap::new(),
            limits: ModelLimits::default(),
            pricing: ModelPricing::default(),
            governance: None,
            clients,
        }
    }

    #[must_use]
    pub fn upstream_name<'a>(&'a self, id: &'a ModelId) -> &'a str {
        self.upstream_model
            .as_deref()
            .unwrap_or_else(|| id.as_str())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ModelRegistry {
    pub models: BTreeMap<ModelId, ModelEntry>,
}

impl ModelRegistry {
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ModelEntry> {
        self.models.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ModelId, &ModelEntry)> {
        self.models.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}
