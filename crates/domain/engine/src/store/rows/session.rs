//! Rows of the `session` table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, Params, Row, params};

use crate::store::error::Result;
use crate::store::id::{ProjectId, SessionId, column, optional_column, optional_param, param};
use crate::store::model::{DiffSummary, Session};
use crate::store::rows::{to_i64, to_u64};

const SESSION_COLUMNS: &str = "id, project_id, parent_id, directory, title, agent, model, \
     permission, revert, additions, deletions, files, metadata, created_at, updated_at, \
     archived_at";

struct SessionRow {
    id: SessionId,
    project_id: ProjectId,
    parent_id: Option<SessionId>,
    directory: String,
    title: String,
    agent: Option<String>,
    model: Option<String>,
    permission: Option<String>,
    revert: Option<String>,
    additions: i64,
    deletions: i64,
    files: i64,
    metadata: String,
    created_at: i64,
    updated_at: i64,
    archived_at: Option<i64>,
}

fn session_row(row: &Row<'_>) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        id: column(row, 0)?,
        project_id: column(row, 1)?,
        parent_id: optional_column(row, 2)?,
        directory: row.get(3)?,
        title: row.get(4)?,
        agent: row.get(5)?,
        model: row.get(6)?,
        permission: row.get(7)?,
        revert: row.get(8)?,
        additions: row.get(9)?,
        deletions: row.get(10)?,
        files: row.get(11)?,
        metadata: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        archived_at: row.get(15)?,
    })
}

impl SessionRow {
    fn decode(self) -> Result<Session> {
        Ok(Session {
            id: self.id,
            project_id: self.project_id,
            parent_id: self.parent_id,
            directory: PathBuf::from(self.directory),
            title: self.title,
            agent: self.agent,
            model: self.model,
            permission: self
                .permission
                .as_deref()
                .map(serde_json::from_str)
                .transpose()?,
            revert: self
                .revert
                .as_deref()
                .map(serde_json::from_str)
                .transpose()?,
            summary: DiffSummary {
                additions: to_u64(self.additions),
                deletions: to_u64(self.deletions),
                files: to_u64(self.files),
            },
            metadata: serde_json::from_str(&self.metadata)?,
            created_at: self.created_at,
            updated_at: self.updated_at,
            archived_at: self.archived_at,
        })
    }
}

pub(crate) fn query_sessions(
    connection: &Connection,
    tail: &str,
    params: impl Params,
) -> Result<Vec<Session>> {
    let mut statement =
        connection.prepare(&format!("SELECT {SESSION_COLUMNS} FROM session {tail}"))?;
    let rows = statement
        .query_map(params, session_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().map(SessionRow::decode).collect()
}

pub(crate) fn load_session(connection: &Connection, id: &SessionId) -> Result<Option<Session>> {
    connection
        .query_row(
            &format!("SELECT {SESSION_COLUMNS} FROM session WHERE id = ?1"),
            params![param(id)],
            session_row,
        )
        .optional()?
        .map(SessionRow::decode)
        .transpose()
}

struct SessionColumns {
    directory: String,
    permission: Option<String>,
    revert: Option<String>,
    metadata: String,
}

impl SessionColumns {
    fn of(session: &Session) -> Result<Self> {
        Ok(Self {
            directory: session.directory.to_string_lossy().into_owned(),
            permission: session
                .permission
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            revert: session
                .revert
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            metadata: serde_json::to_string(&session.metadata)?,
        })
    }
}

pub(crate) fn insert_session(connection: &Connection, session: &Session) -> Result<()> {
    let columns = SessionColumns::of(session)?;
    connection.execute(
        "INSERT INTO session (id, project_id, parent_id, directory, title, agent, model, \
           permission, revert, additions, deletions, files, metadata, created_at, updated_at, \
           archived_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            param(&session.id),
            param(&session.project_id),
            optional_param(session.parent_id.as_ref()),
            columns.directory,
            session.title,
            session.agent,
            session.model,
            columns.permission,
            columns.revert,
            to_i64(session.summary.additions),
            to_i64(session.summary.deletions),
            to_i64(session.summary.files),
            columns.metadata,
            session.created_at,
            session.updated_at,
            session.archived_at,
        ],
    )?;
    Ok(())
}

pub(crate) fn update_session_row(connection: &Connection, session: &Session) -> Result<()> {
    let columns = SessionColumns::of(session)?;
    connection.execute(
        "UPDATE session SET directory = ?2, title = ?3, agent = ?4, model = ?5, permission = ?6, \
           revert = ?7, additions = ?8, deletions = ?9, files = ?10, metadata = ?11, \
           updated_at = ?12, archived_at = ?13 \
         WHERE id = ?1",
        params![
            param(&session.id),
            columns.directory,
            session.title,
            session.agent,
            session.model,
            columns.permission,
            columns.revert,
            to_i64(session.summary.additions),
            to_i64(session.summary.deletions),
            to_i64(session.summary.files),
            columns.metadata,
            session.updated_at,
            session.archived_at,
        ],
    )?;
    Ok(())
}
