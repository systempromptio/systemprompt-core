//! The agent's own presence and caret after a landed write.
//!
//! The presence is filed with source `agent`, so a console shows the person
//! and their assistant side by side, and the caret sits on the first block
//! it wrote, so open tabs see where it is working. Neither is part of the
//! write: a failure is logged and the ack stands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{
    AwarenessCursor, AwarenessMode, AwarenessState, DocumentAwarenessEvent, PresenceBeacon,
    PresenceSource, PresenceState,
};

use super::write::EditCall;
use crate::documents::views::EditOutput;

pub(super) async fn announce_agent(call: &EditCall<'_>, output: &EditOutput) {
    let agent = call.ctx.agent_name().to_string();
    let service = call.tools.service();
    let beacon = PresenceBeacon {
        document_id: call.document.clone(),
        state: PresenceState::Editing,
        source_kind: PresenceSource::Agent,
        source_ref: agent.clone(),
        cursor: None,
    };
    if let Err(error) = service
        .touch_presence(call.who, beacon, call.ctx.trace_id())
        .await
    {
        tracing::warn!(document_id = %call.document, %error, "agent presence not recorded");
    }
    let Some(block) = output
        .written
        .first()
        .and_then(|line| line.split(':').next())
        .and_then(|id| DocumentNodeId::try_new(id).ok())
    else {
        return;
    };
    let cursor = AwarenessCursor {
        block,
        anchor: String::new(),
        head: String::new(),
    };
    let event = DocumentAwarenessEvent {
        document_id: call.document.clone(),
        user_id: call.who.id.clone(),
        display_name: format!("{agent} for {}", call.who.username),
        state: AwarenessState {
            session_id: None,
            color: AGENT_CARET_COLOR.to_owned(),
            mode: AwarenessMode::Editing,
            cursor: Some(cursor),
            selection: Vec::new(),
            viewport: None,
            agent: Some(agent),
        },
        expires_at: Utc::now() + chrono::Duration::seconds(AGENT_CARET_SECS),
    };
    if let Err(error) = service.events().awareness(&event).await {
        tracing::warn!(document_id = %call.document, %error, "agent caret not announced");
    }
}

const AGENT_CARET_COLOR: &str = "tone-agent";
const AGENT_CARET_SECS: i64 = 30;
