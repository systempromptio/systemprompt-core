//! Repositories for task-attached content.
//!
//! Groups persistence for the two kinds of content produced by a task:
//! generated [`ArtifactRepository`] outputs and per-task
//! [`PushNotificationConfigRepository`] delivery configuration.

pub mod artifact;
pub mod push_notification;

pub use artifact::ArtifactRepository;
pub use push_notification::PushNotificationConfigRepository;
