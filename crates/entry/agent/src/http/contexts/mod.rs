//! Context resource handlers for the agent surface: the caller's contexts,
//! their notifications and the events forwarded to their streams. The
//! `/api/v1/core/contexts` route set in [`crate::http`] mounts them with the
//! nested task and artifact listings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod create_context;
pub mod delete_context;
pub mod events;
pub mod get_context;
pub mod list_contexts;
pub mod notifications;
pub mod update_context;

pub use create_context::create_context;
pub use delete_context::delete_context;
pub use get_context::get_context;
pub use list_contexts::list_contexts;
pub use notifications::handle_context_notification;
pub use update_context::update_context;
