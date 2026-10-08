//! Session contracts implemented over [`crate::SessionRepository`]: the
//! analytics session store, the session provider and usage counters, and the
//! AI session provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_provider;
mod providers;
mod store;

pub use ai_provider::UsersAiSessionProvider;
