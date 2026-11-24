//! OAuth dynamic client-registration management routes.
//!
//! Groups the CRUD handlers for registered OAuth clients: [`create`],
//! [`get`], [`list`], [`update`], and [`delete`]. A client is managed by its
//! owner or an admin; a listing shows a non-admin only the clients it owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod create;
pub mod delete;
pub mod get;
pub mod list;
mod ownership;
pub mod update;
