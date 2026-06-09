//! System-prompt overrides for gateway requests.
//!
//! A [`SystemPromptOverride`] inspects an [`OverrideContext`] (the resolved
//! provider, the requested and upstream model, and the current system prompt)
//! and returns an [`OverrideAction`] — replace the system prompt, strip it, or
//! pass through unchanged. The gateway applies the first non-pass-through
//! action at dispatch, before the request is forwarded upstream.
//!
//! Two sources feed the [`engine::OverrideEngine`]: declarative
//! [`SystemPromptRule`](systemprompt_models::profile::SystemPromptRule)
//! rules from the profile (evaluated first), then extension overrides
//! contributed through the
//! [`register_system_prompt_override!`](crate::register_system_prompt_override)
//! macro and collected via `inventory` — the same pattern used for
//! [`SafetyScanner`](super::safety::SafetyScanner)s.

mod engine;

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{ModelId, ProviderId};

pub use engine::OverrideEngine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideAction {
    Replace(String),
    Strip,
    Passthrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideSource {
    Config,
    Extension(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideResolution {
    pub action: OverrideAction,
    pub source: Option<OverrideSource>,
}

impl OverrideResolution {
    #[must_use]
    pub const fn passthrough() -> Self {
        Self {
            action: OverrideAction::Passthrough,
            source: None,
        }
    }

    #[must_use]
    pub fn audit_descriptor(&self) -> Option<String> {
        let action = match self.action {
            OverrideAction::Replace(_) => "replace",
            OverrideAction::Strip => "strip",
            OverrideAction::Passthrough => return None,
        };
        match self.source.as_ref()? {
            OverrideSource::Config => Some(format!("config:{action}")),
            OverrideSource::Extension(name) => Some(format!("extension:{name}:{action}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OverrideContext {
    provider: ProviderId,
    requested_model: ModelId,
    upstream_model: ModelId,
    current_system: Option<String>,
}

impl OverrideContext {
    #[must_use]
    pub const fn builder(provider: ProviderId, requested_model: ModelId) -> OverrideContextBuilder {
        OverrideContextBuilder {
            provider,
            requested_model,
            upstream_model: None,
            current_system: None,
        }
    }

    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    #[must_use]
    pub const fn requested_model(&self) -> &ModelId {
        &self.requested_model
    }

    #[must_use]
    pub const fn upstream_model(&self) -> &ModelId {
        &self.upstream_model
    }

    #[must_use]
    pub fn current_system(&self) -> Option<&str> {
        self.current_system.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct OverrideContextBuilder {
    provider: ProviderId,
    requested_model: ModelId,
    upstream_model: Option<ModelId>,
    current_system: Option<String>,
}

impl OverrideContextBuilder {
    #[must_use]
    pub fn upstream_model(mut self, model: ModelId) -> Self {
        self.upstream_model = Some(model);
        self
    }

    #[must_use]
    pub fn current_system(mut self, system: Option<String>) -> Self {
        self.current_system = system;
        self
    }

    #[must_use]
    pub fn build(self) -> OverrideContext {
        let upstream_model = self
            .upstream_model
            .unwrap_or_else(|| self.requested_model.clone());
        OverrideContext {
            provider: self.provider,
            requested_model: self.requested_model,
            upstream_model,
            current_system: self.current_system,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OverrideError {
    #[error("system-prompt override '{name}' failed: {message}")]
    Failed { name: &'static str, message: String },
}

// Why: #[async_trait] is required — overrides are held as trait objects in the
// engine registry, so the trait must stay dyn-compatible.
#[async_trait]
pub trait SystemPromptOverride: Send + Sync {
    fn name(&self) -> &'static str;
    async fn evaluate(&self, ctx: &OverrideContext) -> Result<OverrideAction, OverrideError>;
}

#[derive(Debug, Clone, Copy)]
pub struct SystemPromptOverrideRegistration {
    pub name: &'static str,
    pub factory: fn() -> Arc<dyn SystemPromptOverride>,
}

inventory::collect!(SystemPromptOverrideRegistration);

/// Register a [`SystemPromptOverride`] implementation with the gateway.
///
/// ```ignore
/// use systemprompt_ai::register_system_prompt_override;
/// register_system_prompt_override!(TenantPromptOverride::new, name = "tenant-prompt");
/// ```
///
/// `$factory` is any `fn() -> Arc<dyn SystemPromptOverride>`.
#[macro_export]
macro_rules! register_system_prompt_override {
    ($factory:expr, name = $name:expr $(,)?) => {
        ::inventory::submit! {
            $crate::SystemPromptOverrideRegistration {
                name: $name,
                factory: || ::std::sync::Arc::new($factory()),
            }
        }
    };
}
