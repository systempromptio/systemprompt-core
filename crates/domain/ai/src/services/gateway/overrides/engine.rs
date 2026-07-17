//! Resolution of the effective system-prompt override.
//!
//! [`OverrideEngine`] evaluates the profile's declarative
//! [`SystemPromptRule`]s first (file order, first match wins), then the
//! `inventory`-registered extension overrides in registration order. The first
//! non-[`OverrideAction::Passthrough`] result wins; an extension that errors is
//! logged and treated as pass-through so a misconfigured override can never
//! fail dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, OnceLock};

use systemprompt_models::profile::{OverrideRuleAction, SystemPromptRule};

use super::{
    OverrideAction, OverrideContext, OverrideResolution, OverrideSource, SystemPromptOverride,
    SystemPromptOverrideRegistration,
};

pub struct OverrideEngine {
    extensions: Vec<Arc<dyn SystemPromptOverride>>,
}

impl std::fmt::Debug for OverrideEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverrideEngine")
            .field("extensions", &self.extensions.len())
            .finish()
    }
}

impl OverrideEngine {
    #[must_use]
    pub fn global() -> &'static Self {
        static ENGINE: OnceLock<OverrideEngine> = OnceLock::new();
        ENGINE.get_or_init(|| Self {
            extensions: inventory::iter::<SystemPromptOverrideRegistration>()
                .map(|reg| (reg.factory)())
                .collect(),
        })
    }

    #[must_use]
    pub fn has_extensions(&self) -> bool {
        !self.extensions.is_empty()
    }

    pub async fn resolve(
        &self,
        rules: &[SystemPromptRule],
        ctx: &OverrideContext,
    ) -> OverrideResolution {
        if let Some(resolution) = Self::resolve_config(rules, ctx) {
            return resolution;
        }
        self.resolve_extensions(ctx).await
    }

    fn resolve_config(
        rules: &[SystemPromptRule],
        ctx: &OverrideContext,
    ) -> Option<OverrideResolution> {
        let rule = rules
            .iter()
            .find(|rule| rule.matches(ctx.provider(), ctx.requested_model().as_str()))?;
        let action = match rule.action {
            OverrideRuleAction::Replace => OverrideAction::Replace(rule.prompt.clone()?),
            OverrideRuleAction::Strip => OverrideAction::Strip,
        };
        tracing::info!(
            source = "config",
            provider = %ctx.provider(),
            model = %ctx.requested_model(),
            action = ?rule.action,
            "gateway system-prompt override applied"
        );
        Some(OverrideResolution {
            action,
            source: Some(OverrideSource::Config),
        })
    }

    async fn resolve_extensions(&self, ctx: &OverrideContext) -> OverrideResolution {
        for ext in &self.extensions {
            match ext.evaluate(ctx).await {
                Ok(OverrideAction::Passthrough) => {},
                Ok(action) => {
                    tracing::info!(
                        source = "extension",
                        override_name = ext.name(),
                        provider = %ctx.provider(),
                        model = %ctx.requested_model(),
                        "gateway system-prompt override applied"
                    );
                    return OverrideResolution {
                        action,
                        source: Some(OverrideSource::Extension(ext.name())),
                    };
                },
                Err(e) => {
                    tracing::warn!(
                        override_name = ext.name(),
                        error = %e,
                        "system-prompt override errored; passing through"
                    );
                },
            }
        }
        OverrideResolution::passthrough()
    }
}
