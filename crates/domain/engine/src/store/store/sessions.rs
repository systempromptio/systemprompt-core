//! Sessions: creation, edits, listings, search and deletion of a whole subagent
//! tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rusqlite::{Connection, params};

use crate::store::change::Change;
use crate::store::error::{Result, not_found};
use crate::store::id::{ProjectId, SessionId, column, now_ms, optional_column, optional_param, param};
use crate::store::model::{DiffSummary, NewSession, RevertMarker, Session};
use crate::store::rows::{insert_session, load_session, query_sessions, update_session_row};
use crate::store::store::Store;

const SESSION_TREE: &str = "WITH RECURSIVE tree(id, depth) AS ( \
       SELECT id, 0 FROM session WHERE id = ?1 \
       UNION SELECT session.id, tree.depth + 1 FROM session \
         JOIN tree ON session.parent_id = tree.id \
     ) SELECT session.id, session.parent_id FROM tree JOIN session ON session.id = tree.id \
     ORDER BY tree.depth DESC";

const SEARCH_TAIL: &str = "WHERE (?1 IS NULL OR project_id = ?1) AND title LIKE ?2 ESCAPE '\\' \
     AND archived_at IS NULL ORDER BY updated_at DESC, id DESC LIMIT ?3";

impl Store {
    pub fn create_session(&self, new: NewSession) -> Result<Session> {
        self.write(|transaction, changes| {
            let now = now_ms();
            let session = Session {
                id: SessionId::mint(),
                project_id: new.project_id,
                parent_id: new.parent_id,
                directory: new.directory,
                title: new.title,
                agent: new.agent,
                model: new.model,
                permission: new.permission,
                revert: None,
                summary: DiffSummary::default(),
                metadata: new.metadata,
                created_at: now,
                updated_at: now,
                archived_at: None,
            };
            insert_session(transaction, &session)?;
            Self::touch_project(transaction, &session.project_id, now)?;
            changes.push(Change::SessionCreated(Box::new(session.clone())));
            Ok(session)
        })
    }

    pub fn get_session(&self, id: &SessionId) -> Result<Option<Session>> {
        self.read(|connection| load_session(connection, id))
    }

    pub fn update_session(
        &self,
        id: &SessionId,
        edit: impl FnOnce(&mut Session),
    ) -> Result<Session> {
        self.write(|transaction, changes| {
            let current = load_session(transaction, id)?.ok_or_else(|| not_found("session", id))?;
            let mut next = current.clone();
            edit(&mut next);
            next.id = current.id;
            next.project_id = current.project_id;
            next.parent_id = current.parent_id;
            next.created_at = current.created_at;
            next.updated_at = now_ms();
            update_session_row(transaction, &next)?;
            changes.push(Change::SessionUpdated(Box::new(next.clone())));
            Ok(next)
        })
    }

    pub fn set_title(&self, id: &SessionId, title: &str) -> Result<Session> {
        self.update_session(id, |session| title.clone_into(&mut session.title))
    }

    pub fn set_summary(&self, id: &SessionId, summary: DiffSummary) -> Result<Session> {
        self.update_session(id, |session| session.summary = summary)
    }

    pub fn set_archived(&self, id: &SessionId, archived: bool) -> Result<Session> {
        self.update_session(id, |session| {
            session.archived_at = if archived {
                Some(session.archived_at.unwrap_or_else(now_ms))
            } else {
                None
            };
        })
    }

    pub fn set_revert(&self, id: &SessionId, marker: RevertMarker) -> Result<Session> {
        self.update_session(id, |session| session.revert = Some(marker))
    }

    pub fn clear_revert(&self, id: &SessionId) -> Result<Session> {
        self.update_session(id, |session| session.revert = None)
    }

    pub fn root_sessions(
        &self,
        project_id: &ProjectId,
        include_archived: bool,
    ) -> Result<Vec<Session>> {
        self.read(|connection| {
            query_sessions(
                connection,
                "WHERE project_id = ?1 AND parent_id IS NULL AND (?2 OR archived_at IS NULL) \
                 ORDER BY updated_at DESC, id DESC",
                params![param(project_id), include_archived],
            )
        })
    }

    pub fn child_sessions(&self, parent_id: &SessionId) -> Result<Vec<Session>> {
        self.read(|connection| {
            query_sessions(
                connection,
                "WHERE parent_id = ?1 ORDER BY created_at, id",
                params![param(parent_id)],
            )
        })
    }

    pub fn search_sessions(
        &self,
        project_id: Option<&ProjectId>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Session>> {
        let pattern = like_pattern(query.trim());
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        self.read(|connection| {
            query_sessions(
                connection,
                SEARCH_TAIL,
                params![optional_param(project_id), pattern, limit],
            )
        })
    }

    pub fn delete_session(&self, id: &SessionId) -> Result<bool> {
        self.write(|transaction, changes| {
            let Some(root) = load_session(transaction, id)? else {
                return Ok(false);
            };
            let tree = session_tree(transaction, id)?;
            for (session_id, _) in &tree {
                transaction.execute(
                    "DELETE FROM session WHERE id = ?1",
                    params![param(session_id)],
                )?;
            }
            tracing::debug!(session = %id, removed = tree.len(), "session tree deleted");
            for (session_id, parent_id) in tree {
                changes.push(Change::SessionDeleted {
                    id: session_id,
                    project_id: root.project_id.clone(),
                    parent_id,
                });
            }
            Ok(true)
        })
    }
}

fn session_tree(
    connection: &Connection,
    id: &SessionId,
) -> Result<Vec<(SessionId, Option<SessionId>)>> {
    let mut statement = connection.prepare(SESSION_TREE)?;
    let tree = statement
        .query_map(params![param(id)], |row| {
            Ok((
                column::<SessionId>(row, 0)?,
                optional_column::<SessionId>(row, 1)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tree)
}

fn like_pattern(query: &str) -> String {
    let mut pattern = String::with_capacity(query.len() + 2);
    pattern.push('%');
    for character in query.chars() {
        if matches!(character, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('%');
    pattern
}
