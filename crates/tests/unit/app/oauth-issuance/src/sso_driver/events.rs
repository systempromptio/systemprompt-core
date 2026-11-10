use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use systemprompt_oauth_issuance::register_auth_event_sink;
use systemprompt_oauth_issuance::sso::SsoHookContext;
use systemprompt_traits::sso::{AuthEventSink, SsoLoginRefused, SsoLoginSuccess};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recorded {
    Success {
        sub: String,
        email: String,
        created: bool,
    },
    Refused {
        sub: Option<String>,
        email: Option<String>,
        reason: &'static str,
    },
}

fn recorded() -> &'static Mutex<Vec<Recorded>> {
    static EVENTS: OnceLock<Mutex<Vec<Recorded>>> = OnceLock::new();
    EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn events_for_sub(sub: &str) -> Vec<Recorded> {
    recorded()
        .lock()
        .expect("events lock")
        .iter()
        .filter(|event| match event {
            Recorded::Success { sub: s, .. } => s == sub,
            Recorded::Refused { sub: s, .. } => s.as_deref() == Some(sub),
        })
        .cloned()
        .collect()
}

pub fn refusals_with_reason(reason: &str) -> usize {
    recorded()
        .lock()
        .expect("events lock")
        .iter()
        .filter(|event| matches!(event, Recorded::Refused { reason: r, .. } if *r == reason))
        .count()
}

#[derive(Debug)]
struct RecordingSink;

#[async_trait]
impl AuthEventSink for RecordingSink {
    async fn on_login_success(&self, event: &SsoLoginSuccess<'_>) {
        recorded()
            .lock()
            .expect("events lock")
            .push(Recorded::Success {
                sub: event.identity.external_sub.clone(),
                email: event.identity.email.clone(),
                created: event.created,
            });
    }

    async fn on_login_refused(&self, event: &SsoLoginRefused<'_>) {
        recorded()
            .lock()
            .expect("events lock")
            .push(Recorded::Refused {
                sub: event.external_sub.map(str::to_owned),
                email: event.email.map(str::to_owned),
                reason: event.reason,
            });
    }
}

fn sink_factory(_ctx: &SsoHookContext) -> Arc<dyn AuthEventSink> {
    Arc::new(RecordingSink)
}

register_auth_event_sink!(sink_factory);
