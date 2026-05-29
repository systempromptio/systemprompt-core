//! Request handlers for the A2A server endpoints.
//!
//! Covers agent-card discovery ([`handle_agent_card`]), the main JSON-RPC
//! request dispatch ([`handle_agent_request`]), and push-notification config
//! management, all sharing the [`AgentHandlerState`] application state.

pub mod card;
pub mod push_notification_config;
pub mod request;
pub mod state;

pub use card::handle_agent_card;
pub use request::handle_agent_request;
pub use state::AgentHandlerState;
