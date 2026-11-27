//! The one call path for every governed moment.
//!
//! The Governor is asked first and stays the sole authority; the extensions
//! the instance admitted then take part in their resolved order. Observers
//! only hear. A reviewer runs only after the Governor allowed a tool call and
//! can only take that away. An augmenter's headers are merged after the
//! Governor authorised a call and never replace a header the call carries.
//! Every reader goes through [`Governed::current`], so a withheld extension
//! contributes nothing anywhere, and an admitted extension whose own
//! generation moved is read again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod contributions;

use std::sync::Arc;

use harness_config::Paths;
use harness_llm::{Endpoint, ModelSpec, ResponseHead, Usage};
use parking_lot::RwLock;

pub(crate) use contributions::Contributions;
use contributions::Reading;

use crate::extension::{
    Contributed, EnabledExtensions, ExtensionSet, PromptContext, PromptSection, Review, add_headers,
};
use crate::governor::{
    GovernError, Governor, ModelCall, ResponseAction, SessionEvent, ToolCall, ToolOutcome,
    UserPrompt, Verdict,
};

pub(crate) struct Governed {
    governor: Arc<dyn Governor>,
    extensions: ExtensionSet,
    paths: Paths,
    current: RwLock<Arc<Contributions>>,
}

impl Governed {
    pub(crate) fn new(governor: Arc<dyn Governor>, extensions: ExtensionSet, paths: Paths) -> Self {
        let enabled = governor.extensions();
        let initial = Contributions::read(Reading {
            extensions: &extensions,
            governor: governor.as_ref(),
            paths: &paths,
            generation: governor.generation(),
            extension_generations: extension_generations(&extensions, enabled.as_ref()),
            enabled,
            revision: 0,
        });
        Self {
            governor,
            extensions,
            paths,
            current: RwLock::new(Arc::new(initial)),
        }
    }

    pub(crate) fn models(&self) -> Vec<ModelSpec> {
        self.governor.models()
    }

    pub(crate) fn generation(&self) -> u64 {
        self.governor.generation()
    }

    pub(crate) fn governed_by(&self) -> Option<String> {
        self.governor.governed_by()
    }

    pub(crate) fn current(&self) -> Arc<Contributions> {
        let generation = self.governor.generation();
        let enabled = self.governor.extensions();
        let extensions = extension_generations(&self.extensions, enabled.as_ref());
        {
            let current = self.current.read();
            if current.is_for(generation, enabled.as_ref(), &extensions) {
                return Arc::clone(&current);
            }
        }
        let mut slot = self.current.write();
        if slot.is_for(generation, enabled.as_ref(), &extensions) {
            return Arc::clone(&slot);
        }
        let fresh = Arc::new(Contributions::read(Reading {
            extensions: &self.extensions,
            governor: self.governor.as_ref(),
            paths: &self.paths,
            generation,
            enabled,
            extension_generations: extensions,
            revision: slot.revision.wrapping_add(1),
        }));
        tracing::info!(
            generation,
            revision = fresh.revision,
            enabled = fresh.enabled.as_ref().map_or(0, EnabledExtensions::len),
            tools = fresh.tools.len(),
            "extension contributions re-read"
        );
        *slot = Arc::clone(&fresh);
        drop(slot);
        fresh
    }

    pub(crate) fn prompt_sections(
        &self,
        step: &PromptContext<'_>,
    ) -> Vec<Contributed<PromptSection>> {
        let current = self.current();
        self.extensions
            .prompt_sections(current.enabled.as_ref(), step)
    }

    pub(crate) async fn authorize(&self, call: &ModelCall) -> Result<Endpoint, GovernError> {
        let mut endpoint = self.governor.authorize(call).await?;
        let current = self.current();
        for augmenter in &current.augmenters {
            let headers = augmenter.item.headers(call).await;
            add_headers(&mut endpoint, &augmenter.extension, headers);
        }
        Ok(endpoint)
    }

    pub(crate) async fn response(
        &self,
        call: &ModelCall,
        status: u16,
        attempt: u32,
    ) -> ResponseAction {
        self.governor.response(call, status, attempt).await
    }

    pub(crate) async fn response_head(&self, call: &ModelCall, head: &ResponseHead) {
        let current = self.current();
        for observer in &current.observers {
            observer.item.response_head(call, head).await;
        }
    }

    pub(crate) async fn step_finished(&self, call: &ModelCall, usage: &Usage) {
        self.governor.step_finished(call, usage).await;
    }

    pub(crate) async fn user_prompt(&self, prompt: &UserPrompt) {
        let current = self.current();
        for observer in &current.observers {
            observer.item.user_prompt(prompt).await;
        }
    }

    pub(crate) async fn tool_before(&self, call: &ToolCall) -> Verdict {
        let verdict = self.governor.tool_before(call).await;
        if let Verdict::Deny { .. } = verdict {
            return verdict;
        }
        let current = self.current();
        for reviewer in &current.reviewers {
            if let Review::Deny { reason } = reviewer.item.review(call).await {
                tracing::info!(
                    session = %call.session_id,
                    tool = %call.tool,
                    extension = %reviewer.extension,
                    "tool call denied by an extension's review"
                );
                return Verdict::Deny { reason };
            }
        }
        Verdict::Allow
    }

    pub(crate) async fn tool_after(&self, call: &ToolCall, outcome: &ToolOutcome) {
        let current = self.current();
        for observer in &current.observers {
            observer.item.tool_after(call, outcome).await;
        }
    }

    pub(crate) fn session_event(&self, event: &SessionEvent) {
        let current = self.current();
        for observer in &current.observers {
            observer.item.session_event(event);
        }
    }
}

fn extension_generations(
    extensions: &ExtensionSet,
    enabled: Option<&EnabledExtensions>,
) -> Vec<u64> {
    extensions
        .admitted(enabled)
        .into_iter()
        .map(|entry| entry.value.generation())
        .collect()
}
