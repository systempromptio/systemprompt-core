//! The brain's part in governed model calls: [`RecallSwitch`] sends
//! `x-systemprompt-memory: off` when the person turned recall off, and
//! [`MemoryObserver`] records the facts a response's
//! `x-systemprompt-memory-ids` header names for its session. Both are held
//! as `dyn` by the engine, hence `#[async_trait]` on their impls.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_engine::run::extension::{CallAugmenter, GovernorObserver, ResponseHead};
use systemprompt_engine::run::{ModelCall, SessionEvent};
use crate::extension::ExtensionSettings;
use sp_protocol::wire::brain::{MEMORY_OFF, parse_memory_ids};
use sp_protocol::wire::headers;

use crate::members::brain::service::BrainService;
use crate::members::brain::settings::BrainSettings;

pub(super) struct RecallSwitch {
    settings: ExtensionSettings,
}

impl RecallSwitch {
    pub(super) const fn new(settings: ExtensionSettings) -> Self {
        Self { settings }
    }

    fn recall(&self) -> bool {
        match self.settings.get::<BrainSettings>() {
            Ok(settings) => settings.recall,
            Err(error) => {
                tracing::warn!(error = %error, "brain settings do not read; recall is sent off");
                false
            },
        }
    }
}

#[async_trait]
impl CallAugmenter for RecallSwitch {
    async fn headers(&self, _call: &ModelCall) -> Vec<(String, String)> {
        if self.recall() {
            Vec::new()
        } else {
            vec![(headers::MEMORY.to_owned(), MEMORY_OFF.to_owned())]
        }
    }
}

pub(super) struct MemoryObserver {
    service: BrainService,
}

impl MemoryObserver {
    pub(super) const fn new(service: BrainService) -> Self {
        Self { service }
    }
}

#[async_trait]
impl GovernorObserver for MemoryObserver {
    fn session_event(&self, event: &SessionEvent) {
        if let SessionEvent::Deleted { session_id } = event {
            self.service.forget_memory(session_id);
        }
    }

    async fn response_head(&self, call: &ModelCall, head: &ResponseHead) {
        let Some(raw) = head.header(headers::MEMORY_IDS) else {
            self.service.record_memory(&call.session_id, Vec::new());
            return;
        };
        match parse_memory_ids(raw) {
            Ok(facts) => self.service.record_memory(&call.session_id, facts),
            Err(error) => {
                tracing::warn!(session = %call.session_id, error = %error, "injected memory ids do not read; not recorded");
            },
        }
    }
}
