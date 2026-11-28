//! Forking: a new root session holding copies of a session's settled history
//! before a cut, with fresh ids in the same order. A reply that is still
//! running is left behind, and message ids named in the session's metadata
//! are renamed to their copies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::store::change::Change;
use crate::store::error::{Result, not_found};
use crate::store::handle::Store;
use crate::store::id::{
    IdClock, MessageId, PartId, SessionId, column, now_ms, optional_param, param,
};
use crate::store::model::{DiffSummary, Message, Session};
use crate::store::rows::{
    insert_session, load_session, query_messages, update_session_row, write_message,
};

const PART_COPIES: &str = "SELECT ordinal, kind, payload, created_at, updated_at FROM part \
     WHERE message_id = ?1 ORDER BY ordinal, id";

const INSERT_PART_COPY: &str = "INSERT INTO part (id, message_id, session_id, ordinal, kind, \
       payload, created_at, updated_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

struct PartCopy {
    ordinal: i64,
    kind: String,
    payload: String,
    created_at: i64,
    updated_at: i64,
}

impl Store {
    pub fn fork_session(&self, id: &SessionId, before: Option<&MessageId>) -> Result<Session> {
        self.write(|transaction, changes| {
            let source = load_session(transaction, id)?.ok_or_else(|| not_found("session", id))?;
            if let Some(cut) = before {
                ensure_in_session(transaction, cut, id)?;
            }
            let now = now_ms();
            let mut fork = Session {
                id: SessionId::mint(self.ids()),
                parent_id: None,
                revert: None,
                summary: DiffSummary::default(),
                created_at: now,
                updated_at: now,
                archived_at: None,
                ..source
            };
            insert_session(transaction, &fork)?;
            let renamed = copy_messages(transaction, self.ids(), id, before, &fork.id)?;
            let copied = renamed.len();
            for value in fork.metadata.values_mut() {
                rename_ids(value, &renamed);
            }
            update_session_row(transaction, &fork)?;
            Self::touch_project(transaction, &fork.project_id, now)?;
            tracing::debug!(source = %id, fork = %fork.id, messages = copied, "session forked");
            changes.push(Change::SessionCreated(Box::new(fork.clone())));
            Ok(fork)
        })
    }
}

fn ensure_in_session(
    connection: &Connection,
    message_id: &MessageId,
    session_id: &SessionId,
) -> Result<()> {
    let owner: Option<SessionId> = connection
        .query_row(
            "SELECT session_id FROM message WHERE id = ?1",
            params![param(message_id)],
            |row| column(row, 0),
        )
        .optional()?;
    if owner.as_ref() == Some(session_id) {
        Ok(())
    } else {
        Err(not_found("message", message_id))
    }
}

fn copy_messages(
    connection: &Connection,
    ids: &IdClock,
    source: &SessionId,
    before: Option<&MessageId>,
    fork_id: &SessionId,
) -> Result<HashMap<MessageId, MessageId>> {
    let messages = query_messages(
        connection,
        "WHERE session_id = ?1 AND (?2 IS NULL OR id < ?2) ORDER BY id",
        params![param(source), optional_param(before)],
    )?;
    let mut renamed: HashMap<MessageId, MessageId> = HashMap::with_capacity(messages.len());
    for message in messages.into_iter().filter(is_settled) {
        let old_id = message.id().clone();
        let new_id = MessageId::mint(ids);
        let copy = retarget(message, &new_id, fork_id, &renamed);
        write_message(connection, &copy)?;
        copy_parts(connection, ids, &old_id, &new_id, fork_id)?;
        renamed.insert(old_id, new_id);
    }
    Ok(renamed)
}

fn is_settled(message: &Message) -> bool {
    message
        .as_assistant()
        .is_none_or(|reply| reply.completed_at.is_some() || reply.error.is_some())
}

// JSON: session metadata is engine bookkeeping; any string in it that names a
// copied message is renamed to the copy.
fn rename_ids(value: &mut Value, renamed: &HashMap<MessageId, MessageId>) {
    match value {
        Value::String(text) => {
            if let Some(copy) = MessageId::parse(text).ok().and_then(|id| renamed.get(&id)) {
                *text = copy.to_string();
            }
        },
        Value::Array(items) => items.iter_mut().for_each(|item| rename_ids(item, renamed)),
        Value::Object(map) => map.values_mut().for_each(|item| rename_ids(item, renamed)),
        Value::Null | Value::Bool(_) | Value::Number(_) => {},
    }
}

fn retarget(
    message: Message,
    new_id: &MessageId,
    fork_id: &SessionId,
    renamed: &HashMap<MessageId, MessageId>,
) -> Message {
    match message {
        Message::User(mut user) => {
            user.id = new_id.clone();
            user.session_id = fork_id.clone();
            Message::User(user)
        },
        Message::Assistant(mut assistant) => {
            assistant.id = new_id.clone();
            assistant.session_id = fork_id.clone();
            if let Some(parent) = renamed.get(&assistant.parent_id) {
                assistant.parent_id = parent.clone();
            }
            Message::Assistant(assistant)
        },
    }
}

fn copy_parts(
    connection: &Connection,
    ids: &IdClock,
    from: &MessageId,
    to: &MessageId,
    session_id: &SessionId,
) -> Result<()> {
    let mut statement = connection.prepare(PART_COPIES)?;
    let parts = statement
        .query_map(params![param(from)], |row| {
            Ok(PartCopy {
                ordinal: row.get(0)?,
                kind: row.get(1)?,
                payload: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for part in parts {
        let part_id = PartId::mint(ids);
        connection.execute(
            INSERT_PART_COPY,
            params![
                param(&part_id),
                param(to),
                param(session_id),
                part.ordinal,
                part.kind,
                part.payload,
                part.created_at,
                part.updated_at,
            ],
        )?;
    }
    Ok(())
}
