//! Data models for the [`super::ServiceRepository`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub module_name: String,
    pub status: String,
    pub pid: Option<i32>,
    pub port: i32,
    pub binary_mtime: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug)]
pub struct CreateServiceInput<'a> {
    pub name: &'a str,
    pub module_name: &'a str,
    pub status: &'a str,
    pub port: u16,
    pub binary_mtime: Option<i64>,
}
