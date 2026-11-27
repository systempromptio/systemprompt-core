//! Rows of the `part` and `todo` tables.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rusqlite::{Connection, OptionalExtension, Params, Row, params};

use crate::store::error::{Result, corrupt};
use crate::store::id::{MessageId, PartId, SessionId, column, param};
use crate::store::model::{Part, Todo, TodoPriority, TodoStatus};

const PART_COLUMNS: &str = "id, session_id, message_id, payload, created_at, updated_at";

const UPSERT_PART: &str = "INSERT INTO part (id, message_id, session_id, ordinal, kind, payload, \
       created_at, updated_at) \
     VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(ordinal), -1) + 1 FROM part WHERE message_id = ?2), \
             ?4, ?5, ?6, ?7) \
     ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, payload = excluded.payload, \
       updated_at = excluded.updated_at";

struct PartRow {
    id: PartId,
    session_id: SessionId,
    message_id: MessageId,
    payload: String,
    created_at: i64,
    updated_at: i64,
}

fn part_row(row: &Row<'_>) -> rusqlite::Result<PartRow> {
    Ok(PartRow {
        id: column(row, 0)?,
        session_id: column(row, 1)?,
        message_id: column(row, 2)?,
        payload: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

impl PartRow {
    fn decode(self) -> Result<Part> {
        Ok(Part {
            id: self.id,
            session_id: self.session_id,
            message_id: self.message_id,
            created_at: self.created_at,
            updated_at: self.updated_at,
            body: serde_json::from_str(&self.payload)?,
        })
    }
}

pub(crate) fn query_parts(
    connection: &Connection,
    tail: &str,
    params: impl Params,
) -> Result<Vec<Part>> {
    let mut statement = connection.prepare(&format!("SELECT {PART_COLUMNS} FROM part {tail}"))?;
    let rows = statement
        .query_map(params, part_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().map(PartRow::decode).collect()
}

pub(crate) fn load_part(connection: &Connection, id: &PartId) -> Result<Option<Part>> {
    connection
        .query_row(
            &format!("SELECT {PART_COLUMNS} FROM part WHERE id = ?1"),
            params![param(id)],
            part_row,
        )
        .optional()?
        .map(PartRow::decode)
        .transpose()
}

pub(crate) fn write_part(connection: &Connection, part: &Part, updated_at: i64) -> Result<()> {
    connection.execute(
        UPSERT_PART,
        params![
            param(&part.id),
            param(&part.message_id),
            param(&part.session_id),
            part.kind(),
            serde_json::to_string(&part.body)?,
            part.created_at,
            updated_at,
        ],
    )?;
    Ok(())
}

pub(crate) fn query_todos(connection: &Connection, session_id: &SessionId) -> Result<Vec<Todo>> {
    let mut statement = connection.prepare(
        "SELECT content, status, priority FROM todo WHERE session_id = ?1 ORDER BY ordinal",
    )?;
    let rows = statement
        .query_map(params![param(session_id)], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(content, status, priority)| {
            let status = TodoStatus::parse(&status)
                .ok_or_else(|| corrupt("todo.status", status.as_str()))?;
            let priority = TodoPriority::parse(&priority)
                .ok_or_else(|| corrupt("todo.priority", priority.as_str()))?;
            Ok(Todo {
                content,
                status,
                priority,
            })
        })
        .collect()
}
