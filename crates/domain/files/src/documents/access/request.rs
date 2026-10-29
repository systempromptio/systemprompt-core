//! The authz request a document access check sends the hook, and the
//! not-found answer for a caller who holds no level.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_governance::authz::{AuthzContext, AuthzRequest, EntityRef};
use systemprompt_identifiers::{Actor, AgentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;

use super::service::Resolved;
use super::subjects::{ROLE_KIND, USER_KIND};
use crate::documents::agents::DocumentActor;
use crate::documents::error::DocumentsError;
use crate::documents::models::SubjectPairs;

pub(super) fn not_found(resolved: &Resolved) -> DocumentsError {
    match (&resolved.document, &resolved.folder) {
        (Some(document), _) => DocumentsError::not_found("document", &document.id),
        (None, Some(folder)) => DocumentsError::not_found("folder", &folder.id),
        (None, None) => DocumentsError::not_found("document", "root"),
    }
}

// JSON: `AuthzRequest.attributes` is the hook's opaque subject bag.
fn subject_attributes(
    who: &AuthenticatedUser,
    pairs: &SubjectPairs,
) -> BTreeMap<String, serde_json::Value> {
    let mut attributes = who.attributes.clone();
    let mut dimensions: BTreeMap<&str, Vec<serde_json::Value>> = BTreeMap::new();
    for (kind, value) in pairs.kinds.iter().zip(&pairs.values) {
        if kind != USER_KIND && kind != ROLE_KIND {
            dimensions
                .entry(kind.as_str())
                .or_default()
                .push(value.as_str().into());
        }
    }
    for (kind, values) in dimensions {
        attributes.insert(kind.to_owned(), serde_json::Value::Array(values));
    }
    attributes
}

pub(super) struct HookTarget<'a> {
    pub(super) who: &'a AuthenticatedUser,
    pub(super) pairs: &'a SubjectPairs,
    pub(super) entity: EntityRef,
    pub(super) context: AuthzContext,
    pub(super) trace: &'a TraceId,
    pub(super) actor: DocumentActor,
}

fn hook_actor(who: &AuthenticatedUser, actor: DocumentActor) -> Actor {
    match actor {
        DocumentActor::Human => Actor::user(who.id.clone()),
        DocumentActor::Agent(agent) => Actor::agent(who.id.clone(), AgentId::new(agent.name)),
        DocumentActor::System { name } => Actor::job(who.id.clone(), name),
    }
}

pub(super) fn authz_request(target: HookTarget<'_>) -> AuthzRequest {
    let HookTarget {
        who,
        pairs,
        entity,
        context,
        trace,
        actor,
    } = target;
    AuthzRequest {
        entity,
        user_id: who.id.clone(),
        actor: Some(hook_actor(who, actor)),
        client_id: None,
        access_scope: None,
        roles: who.roles.clone(),
        attributes: subject_attributes(who, pairs),
        trace_id: trace.clone(),
        session_id: None,
        context,
        context_id: None,
        task_id: None,
        act_chain: Vec::new(),
    }
}
