//! Actor-attribution types for audit and event rows.
//!
//! Every audit-bearing row in the system carries two facts:
//!
//! 1. **Who is accountable** — a real `UserId` referencing the `users` table.
//! 2. **What surface ran on their behalf** — `User` for direct action, or one
//!    of the system-originated surfaces ([`ActorKind::Job`],
//!    [`ActorKind::Mcp`]) that act on a configured owner's behalf.
//!
//! This is the RFC 8693 `sub` / `act` split. Both facts together let us answer
//! "who did this, and how" without forking the users population — there is no
//! separate service-account table, no `users.kind`, no `is_system`. Designated
//! owners are normal admins; what differs is *which* surface ran the action.

mod actor;

pub use actor::{Actor, ActorKind};
