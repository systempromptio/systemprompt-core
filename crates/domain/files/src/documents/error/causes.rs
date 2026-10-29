//! The grouped causes a [`DocumentsError`](super::DocumentsError) carries for
//! access-control and event-relay failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_events::RelayError;
use systemprompt_governance::authz::AuthzError;
use systemprompt_models::authz::SubjectSourceError;

#[derive(Debug, thiserror::Error)]
pub enum AuthzFailure {
    #[error("subject attributes could not be resolved")]
    Subjects(#[from] SubjectSourceError),
    #[error("access control catalog could not be updated")]
    Catalog(#[from] AuthzError),
}

#[derive(Debug, thiserror::Error)]
pub enum EventFailure {
    #[error("event payload could not be serialised")]
    Serialize(#[from] serde_json::Error),
    #[error("event could not be relayed to other replicas")]
    Relay(#[from] RelayError),
}
