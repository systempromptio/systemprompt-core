//! Analytics vocabularies persisted on `user_sessions`, and the pure
//! bot user-agent and IP-range matchers both the request edge and session
//! analytics classify with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod behavioral_bot;
pub mod bot_keywords;
pub mod traffic_class;

pub use behavioral_bot::BehavioralBotReason;
pub use traffic_class::TrafficClass;
