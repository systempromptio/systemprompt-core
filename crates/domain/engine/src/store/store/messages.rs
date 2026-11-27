//! Messages: upserts, paged history and deletion, including a committed revert.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use rusqlite::{Connection, params};

use crate::store::change::Change;
use crate::store::error::Result;
use crate::store::id::{MessageId, SessionId, column, now_ms, optional_param, param};
use crate::store::model::{Message, MessagePage, MessageWithParts, Part};
use crate::store::rows::{load_message, load_session, query_messages, query_parts, write_message};
use crate::store::store::Store;

const PAGE_TAIL: &str =
    "WHERE session_id = ?1 AND (?2 IS NULL OR id < ?2) ORDER BY id DESC LIMIT ?3";

const PAGE_PARTS_TAIL: &str = "WHERE session_id = ?1 AND message_id >= ?2 AND message_id <= ?3 \
     ORDER BY message_id, ordinal, id";

impl Store {
    pub fn upsert_message(&self, message: &Message) -> Result<()> {
        self.write(|transaction, changes| {
            let inserted = write_message(transaction, message)?;
            changes.push(Change::MessageUpdated(message.clone()));
            if inserted {
                transaction.execute(
                    "UPDATE session SET updated_at = ?2 WHERE id = ?1",
                    params![param(message.session_id()), now_ms()],
                )?;
                if let Some(session) = load_session(transaction, message.session_id())? {
                    changes.push(Change::SessionUpdated(Box::new(session)));
                }
            }
            Ok(())
        })
    }

    pub fn get_message(&self, id: &MessageId) -> Result<Option<Message>> {
        self.read(|connection| load_message(connection, id))
    }

    pub fn messages(
        &self,
        session_id: &SessionId,
        page: &MessagePage,
    ) -> Result<Vec<MessageWithParts>> {
        let limit = page
            .limit
            .map_or(-1, |limit| i64::try_from(limit).unwrap_or(i64::MAX));
        self.read(|connection| {
            let transaction = connection.transaction()?;
            let (messages, parts) = read_page(&transaction, session_id, page, limit)?;
            transaction.commit()?;
            Ok(attach_parts(messages, parts))
        })
    }

    pub fn delete_message(&self, id: &MessageId) -> Result<bool> {
        self.write(|transaction, changes| {
            let Some(message) = load_message(transaction, id)? else {
                return Ok(false);
            };
            transaction.execute("DELETE FROM message WHERE id = ?1", params![param(id)])?;
            changes.push(Change::MessageRemoved {
                session_id: message.session_id().clone(),
                message_id: id.clone(),
            });
            Ok(true)
        })
    }

    pub fn delete_messages_from(
        &self,
        session_id: &SessionId,
        from: &MessageId,
    ) -> Result<Vec<MessageId>> {
        self.write(|transaction, changes| {
            let removed = message_ids_from(transaction, session_id, from)?;
            transaction.execute(
                "DELETE FROM message WHERE session_id = ?1 AND id >= ?2",
                params![param(session_id), param(from)],
            )?;
            for message_id in &removed {
                changes.push(Change::MessageRemoved {
                    session_id: session_id.clone(),
                    message_id: message_id.clone(),
                });
            }
            Ok(removed)
        })
    }
}

fn read_page(
    connection: &Connection,
    session_id: &SessionId,
    page: &MessagePage,
    limit: i64,
) -> Result<(Vec<Message>, Vec<Part>)> {
    let mut messages = query_messages(
        connection,
        PAGE_TAIL,
        params![
            param(session_id),
            optional_param(page.before.as_ref()),
            limit
        ],
    )?;
    messages.reverse();
    let (Some(first), Some(last)) = (messages.first(), messages.last()) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let parts = query_parts(
        connection,
        PAGE_PARTS_TAIL,
        params![param(session_id), param(first.id()), param(last.id())],
    )?;
    Ok((messages, parts))
}

fn attach_parts(messages: Vec<Message>, parts: Vec<Part>) -> Vec<MessageWithParts> {
    let mut grouped: HashMap<MessageId, Vec<Part>> = HashMap::new();
    for part in parts {
        grouped
            .entry(part.message_id.clone())
            .or_default()
            .push(part);
    }
    messages
        .into_iter()
        .map(|message| {
            let parts = grouped.remove(message.id()).unwrap_or_default();
            MessageWithParts { message, parts }
        })
        .collect()
}

fn message_ids_from(
    connection: &Connection,
    session_id: &SessionId,
    from: &MessageId,
) -> Result<Vec<MessageId>> {
    let mut statement = connection
        .prepare("SELECT id FROM message WHERE session_id = ?1 AND id >= ?2 ORDER BY id")?;
    let ids = statement
        .query_map(params![param(session_id), param(from)], |row| {
            column::<MessageId>(row, 0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}
