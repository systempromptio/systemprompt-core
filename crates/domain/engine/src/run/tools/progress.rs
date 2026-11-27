//! A tool call's stored part: the record each state is written through, and the
//! live progress a running tool reports into it until its final state is
//! written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::store::{JsonMap, MessageId, Part, PartBody, SessionId, Store, ToolState};
use crate::tools::Progress;
use parking_lot::Mutex;
use serde_json::Value;
use systemprompt_identifiers::engine::CallId;

use crate::run::db::Db;
use crate::run::session::stream::PendingCall;

#[derive(Debug, Clone)]
pub(crate) struct CallRecord {
    template: Part,
    call_id: CallId,
    tool: String,
}

impl CallRecord {
    pub(crate) fn new(
        session_id: SessionId,
        message_id: MessageId,
        call: &PendingCall,
        started: i64,
    ) -> Self {
        let template = Part {
            id: call.part_id.clone(),
            session_id,
            message_id,
            created_at: started,
            updated_at: started,
            body: PartBody::StepStart { snapshot: None },
        };
        Self {
            template,
            call_id: call.id.clone(),
            tool: call.name.clone(),
        }
    }

    pub(crate) fn part(&self, state: ToolState) -> Part {
        let mut part = self.template.clone();
        let (call_id, tool) = (self.call_id.clone(), self.tool.clone());
        part.body = PartBody::Tool {
            call_id,
            tool,
            state: Box::new(state),
        };
        part
    }

    pub(crate) async fn write(&self, db: &Db, state: ToolState) {
        let part = self.part(state);
        let id = part.id.clone();
        if let Err(error) = db.run(move |store| store.upsert_part(&part)).await {
            tracing::warn!(part = %id, error = %error, "tool call state not recorded");
        }
    }
}

pub(crate) struct PartProgress {
    store: Store,
    record: CallRecord,
    // JSON: the tool arguments as the model wrote them, kept on every progress write.
    input: Value,
    started: i64,
    finished: Mutex<bool>,
}

impl PartProgress {
    // JSON: the tool arguments as the model wrote them, kept on every progress
    // write.
    pub(crate) const fn new(store: Store, record: CallRecord, input: Value, started: i64) -> Self {
        Self {
            store,
            record,
            input,
            started,
            finished: Mutex::new(false),
        }
    }

    pub(crate) fn finish(&self) {
        *self.finished.lock() = true;
    }
}

impl Progress for PartProgress {
    // JSON: live metadata of a running call, as the tool reports it.
    fn update(&self, title: &str, metadata: Value) {
        let finished = self.finished.lock();
        if *finished {
            return;
        }
        let part = self.record.part(ToolState::Running {
            input: self.input.clone(),
            title: Some(title.to_owned()),
            metadata: object(metadata),
            started: self.started,
        });
        if let Err(error) = self.store.upsert_part(&part) {
            tracing::debug!(part = %part.id, error = %error, "tool progress not recorded");
        }
        drop(finished);
    }
}

// JSON: tool metadata as the tool produced it, stored as the part's metadata
// object.
pub(crate) fn object(value: Value) -> JsonMap {
    match value {
        Value::Object(map) => map,
        Value::Null => JsonMap::new(),
        other => {
            let mut map = JsonMap::new();
            map.insert("value".to_owned(), other);
            map
        },
    }
}
