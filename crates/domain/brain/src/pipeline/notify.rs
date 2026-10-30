//! The notify stage, run inside the event's savepoint so a notification
//! commits or rolls back with the fact it reports.
//!
//! A proposed fact emits `brain.fact.proposed`, a dispute
//! `brain.fact.disputed`, and each matching interest that asks for alerts
//! `brain.interest.alert` to its owner. The fact's source is the
//! notification's subject, so recipients are gated by the source's
//! visibility. Without an available sink the stage only logs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use serde_json::json;
use sqlx::PgConnection;
use systemprompt_extension::notifications::DynNotificationSink;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::brain::{FactCandidate, FactStatus, InterestDecl, InterestDelivery};
use systemprompt_models::notifications::{NotificationEvent, NotificationKind};

use super::persist::{Effect, Persisted};
use crate::error::BrainResult;

pub const FACT_PROPOSED: &str = "brain.fact.proposed";
pub const FACT_DISPUTED: &str = "brain.fact.disputed";
pub const INTEREST_ALERT: &str = "brain.interest.alert";
const BODY_CHARS: usize = 200;

/// The stored fact a notification reports.
#[derive(Debug, Clone, Copy)]
pub struct Notice<'a> {
    pub actor: &'a UserId,
    pub candidate: &'a FactCandidate,
    pub statement: &'a str,
    pub status: FactStatus,
    pub persisted: &'a Persisted,
    pub interests: &'a [&'a InterestDecl],
}

pub async fn notify(
    sink: Option<&DynNotificationSink>,
    conn: &mut PgConnection,
    notice: &Notice<'_>,
) -> BrainResult<()> {
    let Some(fact) = notice.persisted.fact.as_ref() else {
        return Ok(());
    };
    let events = events_for(notice, fact);
    if events.is_empty() {
        return Ok(());
    }
    let Some(sink) = sink.filter(|sink| sink.is_available()) else {
        tracing::debug!(fact_id = %fact, events = events.len(), "Brain notifications unavailable");
        return Ok(());
    };
    for event in &events {
        sink.emit(&mut *conn, event).await?;
    }
    Ok(())
}

#[must_use]
pub fn events_for(notice: &Notice<'_>, fact: &BrainFactId) -> Vec<NotificationEvent> {
    let stored = matches!(notice.persisted.effect, Effect::Created | Effect::Revised);
    let mut events = Vec::new();
    if notice.persisted.effect == Effect::Disputed {
        events.extend(event(FACT_DISPUTED, notice, fact));
    } else if stored && notice.status == FactStatus::Proposed {
        events.extend(event(FACT_PROPOSED, notice, fact));
    }
    if stored {
        for interest in notice
            .interests
            .iter()
            .filter(|i| i.delivery.contains(&InterestDelivery::Alert))
        {
            events.extend(event(INTEREST_ALERT, notice, fact).map(|e| {
                e.with_direct_recipient(interest.owner.clone(), None)
                    .with_payload(payload(notice, fact, Some(interest)))
            }));
        }
    }
    events
}

fn event(kind: &str, notice: &Notice<'_>, fact: &BrainFactId) -> Option<NotificationEvent> {
    let kind = match NotificationKind::new(kind) {
        Ok(kind) => kind,
        Err(error) => {
            tracing::error!(error = %error, "Brain notification kind is invalid");
            return None;
        },
    };
    let body: String = notice.statement.chars().take(BODY_CHARS).collect();
    Some(
        NotificationEvent::new(kind, notice.actor.clone(), Utc::now())
            .with_subjects(notice.candidate.source.visibility_subject())
            .with_body(body)
            .with_payload(payload(notice, fact, None)),
    )
}

// JSON: a notification payload is the event's free-form contract field.
fn payload(
    notice: &Notice<'_>,
    fact: &BrainFactId,
    interest: Option<&InterestDecl>,
) -> serde_json::Value {
    json!({
        "fact_id": fact.as_str(),
        "kind": notice.candidate.kind.as_str(),
        "subject": notice.candidate.subject.to_string(),
        "statement": notice.statement,
        "interest": interest.map(|i| i.id.as_str()),
    })
}
