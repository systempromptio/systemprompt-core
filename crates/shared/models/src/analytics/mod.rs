//! Analytics vocabularies persisted on `user_sessions`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod behavioral_bot;
pub mod traffic_class;

pub use behavioral_bot::BehavioralBotReason;
pub use traffic_class::TrafficClass;
