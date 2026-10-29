//! The authz request a document access check sends the hook, and the
//! not-found answer for a caller who holds no level.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_governance::authz::{AuthzContext, AuthzRequest, EntityRef};
use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;

use super::service::Resolved;
use crate::documents::error::DocumentsError;

pub(super) fn not_found(resolved: &Resolved) -> DocumentsError {
    match (&resolved.document, &resolved.folder) {
        (Some(document), _) => DocumentsError::not_found("document", &document.id),
        (None, Some(folder)) => DocumentsError::not_found("folder", &folder.id),
        (None, None) => DocumentsError::not_found("document", "root"),
    }
}

pub(super) fn authz_request(
    who: &AuthenticatedUser,
    entity: EntityRef,
    context: AuthzContext,
    trace: &TraceId,
) -> AuthzRequest {
    AuthzRequest {
        entity,
        user_id: who.id.clone(),
        actor: None,
        client_id: None,
        access_scope: None,
        roles: who.roles.clone(),
        attributes: who.attributes.clone(),
        trace_id: trace.clone(),
        session_id: None,
        context,
        context_id: None,
        task_id: None,
        act_chain: Vec::new(),
    }
}
