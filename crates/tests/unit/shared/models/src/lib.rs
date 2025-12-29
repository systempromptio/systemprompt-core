//! Unit tests for systemprompt-models crate
//!
//! Tests cover:
//! - A2A protocol models (AgentCard, Task, Message, etc.)
//! - AI service models (AiMessage, AiRequest, MessageRole)
//! - Artifact types (ArtifactType, ChartType, ColumnType, etc.)
//! - Configuration models (Environment)
//! - Event system models (SystemEventType, A2AEventType)
//! - Authentication models (BaseRoles, AuthError)

#[cfg(test)]
mod a2a;

#[cfg(test)]
mod ai;

#[cfg(test)]
mod artifacts;

#[cfg(test)]
mod config;

#[cfg(test)]
mod events;

#[cfg(test)]
mod auth;
