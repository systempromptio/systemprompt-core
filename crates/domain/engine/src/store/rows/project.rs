//! Rows of the `project` table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, Params, Row, params};

use crate::store::error::Result;
use crate::store::id::{ProjectId, column, param};
use crate::store::model::Project;

const PROJECT_COLUMNS: &str = "id, worktree, name, created_at, updated_at";

fn project_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: column(row, 0)?,
        worktree: PathBuf::from(row.get::<_, String>(1)?),
        name: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

pub(crate) fn query_projects(
    connection: &Connection,
    tail: &str,
    params: impl Params,
) -> Result<Vec<Project>> {
    let mut statement =
        connection.prepare(&format!("SELECT {PROJECT_COLUMNS} FROM project {tail}"))?;
    let projects = statement
        .query_map(params, project_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(projects)
}

pub(crate) fn load_project(connection: &Connection, id: &ProjectId) -> Result<Option<Project>> {
    Ok(connection
        .query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM project WHERE id = ?1"),
            params![param(id)],
            project_row,
        )
        .optional()?)
}

pub(crate) fn insert_project(connection: &Connection, project: &Project) -> Result<()> {
    connection.execute(
        "INSERT INTO project (id, worktree, name, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            param(&project.id),
            project.worktree.to_string_lossy().into_owned(),
            project.name,
            project.created_at,
            project.updated_at,
        ],
    )?;
    Ok(())
}
