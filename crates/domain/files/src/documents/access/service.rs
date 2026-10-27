//! [`DocumentAccessService`]: grant resolution followed by the authz hook.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_governance::authz::{
    AccessControlRepository, AuthzContext, AuthzDecision, AuthzRequest, EntityRef, SharedAuthzHook,
};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::authz::SubjectAttributeSource;
use systemprompt_models::documents::{AccessLevel, DocumentAction};
use uuid::Uuid;

use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentFolder, SubjectPairs};
use crate::documents::repository::{
    DocumentFolderRepository, DocumentGrantRepository, DocumentRepository,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessTarget {
    Document(DocumentId),
    Folder(Option<DocumentFolderId>),
}

#[derive(Debug, Clone)]
pub struct Granted {
    pub level: AccessLevel,
    pub pairs: SubjectPairs,
    pub document: Option<Document>,
    pub folder: Option<DocumentFolder>,
}

#[must_use]
pub fn root_folder_entity() -> DocumentFolderId {
    DocumentFolderId::from_uuid(Uuid::nil())
}

pub(super) fn no_document() -> DocumentId {
    DocumentId::from_uuid(Uuid::nil())
}

#[derive(Debug, Clone, Copy)]
pub struct AccessCheck<'a> {
    pub who: &'a AuthenticatedUser,
    pub target: &'a AccessTarget,
    pub action: DocumentAction,
    pub trace: &'a TraceId,
}

#[derive(Debug, Clone)]
pub struct DocumentAccessRepositories {
    pub grants: Arc<DocumentGrantRepository>,
    pub folders: Arc<DocumentFolderRepository>,
    pub documents: Arc<DocumentRepository>,
    pub access_entities: Arc<AccessControlRepository>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookMode {
    Evaluate,
    Probe,
}

pub(super) struct Resolved {
    pub(super) level: Option<AccessLevel>,
    pub(super) document: Option<Document>,
    pub(super) folder: Option<DocumentFolder>,
    pub(super) nearest_folder: Option<DocumentFolderId>,
}

/// Decides whether a user may perform a [`DocumentAction`] on a document or
/// folder; see the module docs of `documents::access` for the rules.
pub struct DocumentAccessService {
    pub(super) grants: Arc<DocumentGrantRepository>,
    pub(super) folders: Arc<DocumentFolderRepository>,
    pub(super) documents: Arc<DocumentRepository>,
    pub(super) access_entities: Arc<AccessControlRepository>,
    pub(super) hook: SharedAuthzHook,
    pub(super) subjects: Arc<dyn SubjectAttributeSource>,
}

impl std::fmt::Debug for DocumentAccessService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentAccessService")
            .field("hook", &self.hook)
            .finish_non_exhaustive()
    }
}

impl DocumentAccessService {
    pub fn new(
        repositories: DocumentAccessRepositories,
        hook: SharedAuthzHook,
        subjects: Arc<dyn SubjectAttributeSource>,
    ) -> Self {
        let DocumentAccessRepositories {
            grants,
            folders,
            documents,
            access_entities,
        } = repositories;
        Self {
            grants,
            folders,
            documents,
            access_entities,
            hook,
            subjects,
        }
    }

    pub async fn authorize(
        &self,
        who: &AuthenticatedUser,
        target: AccessTarget,
        action: DocumentAction,
        trace: &TraceId,
    ) -> Result<Granted, DocumentsError> {
        let pairs = self.subject_pairs(who).await?;
        let check = AccessCheck {
            who,
            target: &target,
            action,
            trace,
        };
        self.decide(check, pairs, HookMode::Evaluate).await
    }

    pub async fn probe(
        &self,
        who: &AuthenticatedUser,
        target: AccessTarget,
        action: DocumentAction,
        trace: &TraceId,
    ) -> Result<Granted, DocumentsError> {
        let pairs = self.subject_pairs(who).await?;
        let check = AccessCheck {
            who,
            target: &target,
            action,
            trace,
        };
        self.decide(check, pairs, HookMode::Probe).await
    }

    pub async fn probe_with(
        &self,
        check: AccessCheck<'_>,
        pairs: &SubjectPairs,
    ) -> Result<Granted, DocumentsError> {
        self.decide(check, pairs.clone(), HookMode::Probe).await
    }

    pub async fn probe_folder(
        &self,
        who: &AuthenticatedUser,
        folder: Option<&DocumentFolderId>,
        trace: &TraceId,
    ) -> bool {
        let entity = folder.cloned().unwrap_or_else(root_folder_entity);
        let request = authz_request(
            who,
            EntityRef::Folder(entity),
            AuthzContext::document_access(DocumentAction::View, None, folder),
            trace,
        );
        match self.hook.probe(request).await {
            AuthzDecision::Allow => true,
            AuthzDecision::Deny { reason, policy } => {
                tracing::debug!(
                    folder_id = folder.map(DocumentFolderId::as_str),
                    policy = %policy,
                    reason = %reason,
                    "Document listing withheld a folder the authz hook denies"
                );
                false
            },
        }
    }

    async fn decide(
        &self,
        check: AccessCheck<'_>,
        pairs: SubjectPairs,
        mode: HookMode,
    ) -> Result<Granted, DocumentsError> {
        let AccessCheck {
            who,
            target,
            action,
            trace,
        } = check;
        let resolved = self.resolve(who, target, action, &pairs).await?;
        let needed = action.required_level();
        let level = match resolved.level {
            Some(level) if level >= needed => level,
            held => {
                return Err(DocumentsError::Forbidden {
                    action,
                    needed,
                    held,
                    policy: None,
                });
            },
        };
        let document_id = resolved.document.as_ref().map(|d| d.id.clone());
        let entity = EntityRef::Folder(
            resolved
                .nearest_folder
                .clone()
                .unwrap_or_else(root_folder_entity),
        );
        let context = AuthzContext::document_access(
            action,
            document_id.as_ref(),
            resolved.nearest_folder.as_ref(),
        );
        let request = authz_request(who, entity, context, trace);
        let decision = match mode {
            HookMode::Evaluate => self.hook.evaluate(request).await,
            HookMode::Probe => self.hook.probe(request).await,
        };
        match decision {
            AuthzDecision::Allow => Ok(Granted {
                level,
                pairs,
                document: resolved.document,
                folder: resolved.folder,
            }),
            AuthzDecision::Deny { reason, policy } => {
                tracing::info!(
                    user_id = %who.id,
                    action = %action,
                    document_id = document_id.as_ref().map(DocumentId::as_str),
                    folder_id = resolved.nearest_folder.as_ref().map(DocumentFolderId::as_str),
                    policy = %policy,
                    reason = %reason,
                    "Document access denied by the authz hook"
                );
                Err(DocumentsError::Forbidden {
                    action,
                    needed,
                    held: Some(level),
                    policy: Some(policy),
                })
            },
        }
    }
}

fn authz_request(
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
