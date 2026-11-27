//! Parts and todo lists: part upserts, streamed text deltas and whole-list todo
//! replacement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rusqlite::{OptionalExtension, params};

use crate::change::Change;
use crate::error::{Result, StoreError, not_found};
use crate::id::{MessageId, PartId, SessionId, column, now_ms, param};
use crate::model::{Part, PartBody, Todo};
use crate::rows::{load_part, load_session, query_parts, query_todos, write_part};
use crate::store::Store;

const APPEND_TEXT: &str = "UPDATE part SET payload = json_set(payload, '$.text', \
       COALESCE(json_extract(payload, '$.text'), '') || ?2), updated_at = ?3 \
     WHERE id = ?1";

impl Store {
    pub fn upsert_part(&self, part: &Part) -> Result<Part> {
        self.write(|transaction, changes| {
            write_part(transaction, part, now_ms())?;
            let stored =
                load_part(transaction, &part.id)?.ok_or_else(|| not_found("part", &part.id))?;
            changes.push(Change::PartUpdated(stored.clone()));
            Ok(stored)
        })
    }

    pub fn get_part(&self, id: &PartId) -> Result<Option<Part>> {
        self.read(|connection| load_part(connection, id))
    }

    pub fn parts(&self, message_id: &MessageId) -> Result<Vec<Part>> {
        self.read(|connection| {
            query_parts(
                connection,
                "WHERE message_id = ?1 ORDER BY ordinal, id",
                params![param(message_id)],
            )
        })
    }

    pub fn append_text(&self, part_id: &PartId, delta: &str) -> Result<()> {
        if delta.is_empty() {
            return Ok(());
        }
        self.write(|transaction, changes| {
            let found: Option<(SessionId, MessageId, String)> = transaction
                .query_row(
                    "SELECT session_id, message_id, kind FROM part WHERE id = ?1",
                    params![param(part_id)],
                    |row| {
                        Ok((
                            column::<SessionId>(row, 0)?,
                            column::<MessageId>(row, 1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?;
            let Some((session_id, message_id, kind)) = found else {
                return Err(not_found("part", part_id));
            };
            if kind != PartBody::TEXT && kind != PartBody::REASONING {
                return Err(StoreError::NotText {
                    id: part_id.to_string(),
                });
            }
            transaction.execute(APPEND_TEXT, params![param(part_id), delta, now_ms()])?;
            changes.push(Change::PartDelta {
                session_id,
                message_id,
                part_id: part_id.clone(),
                delta: delta.to_owned(),
            });
            Ok(())
        })
    }

    pub fn delete_part(&self, id: &PartId) -> Result<bool> {
        self.write(|transaction, changes| {
            let Some(part) = load_part(transaction, id)? else {
                return Ok(false);
            };
            transaction.execute("DELETE FROM part WHERE id = ?1", params![param(id)])?;
            changes.push(Change::PartRemoved {
                session_id: part.session_id,
                message_id: part.message_id,
                part_id: part.id,
            });
            Ok(true)
        })
    }

    pub fn todos(&self, session_id: &SessionId) -> Result<Vec<Todo>> {
        self.read(|connection| query_todos(connection, session_id))
    }

    pub fn replace_todos(&self, session_id: &SessionId, todos: &[Todo]) -> Result<()> {
        self.write(|transaction, changes| {
            if load_session(transaction, session_id)?.is_none() {
                return Err(not_found("session", session_id));
            }
            transaction.execute(
                "DELETE FROM todo WHERE session_id = ?1",
                params![param(session_id)],
            )?;
            for (ordinal, todo) in todos.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO todo (session_id, ordinal, content, status, priority) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        param(session_id),
                        i64::try_from(ordinal).unwrap_or(i64::MAX),
                        todo.content,
                        todo.status.as_str(),
                        todo.priority.as_str(),
                    ],
                )?;
            }
            changes.push(Change::TodosUpdated {
                session_id: session_id.clone(),
                todos: todos.to_vec(),
            });
            Ok(())
        })
    }
}
