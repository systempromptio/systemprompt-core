//! Projects: one per worktree, created on first use.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use rusqlite::{Connection, params};

use crate::change::Change;
use crate::error::{Result, not_found};
use crate::id::{ProjectId, SessionId, column, now_ms, optional_column, param};
use crate::model::Project;
use crate::rows::{insert_project, load_project, query_projects};
use crate::store::Store;

impl Store {
    pub fn ensure_project(&self, worktree: &Path, name: &str) -> Result<Project> {
        let key = worktree.to_string_lossy().into_owned();
        self.write(|transaction, changes| {
            let existing = query_projects(transaction, "WHERE worktree = ?1", params![key])?;
            if let Some(existing) = existing.into_iter().next() {
                return Ok(existing);
            }
            let now = now_ms();
            let project = Project {
                id: ProjectId::mint(),
                worktree: worktree.to_path_buf(),
                name: name.to_owned(),
                created_at: now,
                updated_at: now,
            };
            insert_project(transaction, &project)?;
            changes.push(Change::ProjectUpdated(project.clone()));
            Ok(project)
        })
    }

    pub fn get_project(&self, id: &ProjectId) -> Result<Option<Project>> {
        self.read(|connection| load_project(connection, id))
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        self.read(|connection| {
            query_projects(connection, "ORDER BY updated_at DESC, id DESC", params![])
        })
    }

    pub fn rename_project(&self, id: &ProjectId, name: &str) -> Result<Project> {
        self.write(|transaction, changes| {
            let updated = transaction.execute(
                "UPDATE project SET name = ?2, updated_at = ?3 WHERE id = ?1",
                params![param(id), name, now_ms()],
            )?;
            if updated == 0 {
                return Err(not_found("project", id));
            }
            let project = load_project(transaction, id)?.ok_or_else(|| not_found("project", id))?;
            changes.push(Change::ProjectUpdated(project.clone()));
            Ok(project)
        })
    }

    pub fn delete_project(&self, id: &ProjectId) -> Result<bool> {
        self.write(|transaction, changes| {
            let sessions = project_sessions(transaction, id)?;
            let removed =
                transaction.execute("DELETE FROM project WHERE id = ?1", params![param(id)])?;
            if removed == 0 {
                return Ok(false);
            }
            for (session_id, parent_id) in sessions {
                changes.push(Change::SessionDeleted {
                    id: session_id,
                    project_id: id.clone(),
                    parent_id,
                });
            }
            changes.push(Change::ProjectDeleted { id: id.clone() });
            tracing::debug!(project = %id, "project deleted");
            Ok(true)
        })
    }

    pub(crate) fn touch_project(transaction: &Connection, id: &ProjectId, now: i64) -> Result<()> {
        transaction.execute(
            "UPDATE project SET updated_at = ?2 WHERE id = ?1",
            params![param(id), now],
        )?;
        Ok(())
    }
}

fn project_sessions(
    connection: &Connection,
    id: &ProjectId,
) -> Result<Vec<(SessionId, Option<SessionId>)>> {
    let mut statement =
        connection.prepare("SELECT id, parent_id FROM session WHERE project_id = ?1")?;
    let sessions = statement
        .query_map(params![param(id)], |row| {
            Ok((
                column::<SessionId>(row, 0)?,
                optional_column::<SessionId>(row, 1)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(sessions)
}
