//! The document store handles a host binds after its kernel resolves.
//!
//! The document store is built from slot outputs of the same resolve the
//! files member provides into, so the member cannot read the store while it
//! provides. The host creates the cell instead, with
//! [`DocumentsHandlesSlot::unbound`]: it seeds the read half,
//! [`DocumentsHandlesSlot`], into its kernel state before any member
//! provides, and keeps the bind half, [`DocumentsHandlesBinder`], outside the
//! state. The files member's core seams hold the read half and read it when
//! they are called; the host binds the cell once, after it has built the store
//! from the resolved state. No member can bind it: the binder is never in the
//! state, a binder from another `unbound` call binds only its own cell, and a
//! member cannot provide the slot type the kernel already holds. A call
//! before the host binds the slot fails with [`DocumentsUnbound`], never with
//! a default verdict.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::brain::{
    SourceAccess, SourceAccessError, SourceContent, SourceContentError, SourceVerdict,
};
use systemprompt_extension::{
    NotificationAudienceResolver, ResourceVisibility, Visibility, VisibilityError,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{DOCUMENT_SUBJECT_KIND, FactSource, SourceText};
use systemprompt_models::notifications::SubjectRef;
use systemprompt_models::{AlreadyBound, LateBound};

use super::repository::DocumentRepositories;
use super::{
    DocumentAccessService, DocumentService, DocumentSourceAccess, DocumentSourceContent,
    DocumentVisibility,
};
use crate::issues::{ISSUE_SUBJECT, IssueService, IssueVisibility};

/// The document store a host built from its resolved kernel state.
#[derive(Clone)]
pub struct DocumentsHandles {
    pub access: Arc<DocumentAccessService>,
    pub repositories: Arc<DocumentRepositories>,
    pub issues: Arc<IssueService>,
    pub service: Option<Arc<DocumentService>>,
}

impl std::fmt::Debug for DocumentsHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsHandles")
            .field("service", &self.service.is_some())
            .finish_non_exhaustive()
    }
}

/// The read half of the document store cell the host seeds; see the module
/// docs.
#[derive(Clone)]
pub struct DocumentsHandlesSlot(LateBound<DocumentsHandles>);

/// The bind half of the document store cell, held by the host alone.
pub struct DocumentsHandlesBinder(LateBound<DocumentsHandles>);

impl DocumentsHandlesSlot {
    #[must_use]
    pub fn unbound() -> (Self, DocumentsHandlesBinder) {
        let cell = LateBound::default();
        (Self(cell.clone()), DocumentsHandlesBinder(cell))
    }

    #[must_use]
    pub fn get(&self) -> Option<&Arc<DocumentsHandles>> {
        self.0.get()
    }
}

impl DocumentsHandlesBinder {
    pub fn bind(&self, handles: Arc<DocumentsHandles>) -> Result<(), AlreadyBound> {
        self.0.bind(handles)
    }
}

impl std::fmt::Debug for DocumentsHandlesSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsHandlesSlot")
            .field("bound", &self.0.is_bound())
            .finish()
    }
}

impl std::fmt::Debug for DocumentsHandlesBinder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsHandlesBinder")
            .field("bound", &self.0.is_bound())
            .finish()
    }
}

/// The host has not bound the document store yet.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("the document store is not bound on this host")]
pub struct DocumentsUnbound;

pub fn bound(slot: &DocumentsHandlesSlot) -> Result<&Arc<DocumentsHandles>, DocumentsUnbound> {
    slot.get().ok_or(DocumentsUnbound)
}

pub fn bound_service(
    slot: &DocumentsHandlesSlot,
) -> Result<Option<Arc<DocumentService>>, DocumentsUnbound> {
    Ok(bound(slot)?.service.clone())
}

const DOCUMENT_KINDS: &[&str] = &["document", "folder", "comment"];

#[derive(Debug, Clone)]
pub(crate) struct BoundDocumentVisibility(pub(crate) DocumentsHandlesSlot);

impl BoundDocumentVisibility {
    fn current(&self) -> Result<DocumentVisibility, VisibilityError> {
        let handles = bound(&self.0).map_err(|error| VisibilityError::Other {
            kind: "document".to_owned(),
            source: Box::new(error),
        })?;
        Ok(DocumentVisibility::new(
            Arc::clone(&handles.access),
            Arc::clone(&handles.repositories),
        ))
    }
}

#[async_trait]
impl ResourceVisibility for BoundDocumentVisibility {
    fn kinds(&self) -> &'static [&'static str] {
        DOCUMENT_KINDS
    }

    async fn visible(
        &self,
        subject: &SubjectRef,
        reader: &UserId,
    ) -> Result<Visibility, VisibilityError> {
        self.current()?.visible(subject, reader).await
    }

    async fn visible_many(
        &self,
        subject: &SubjectRef,
        readers: &[UserId],
    ) -> Result<Vec<Visibility>, VisibilityError> {
        self.current()?.visible_many(subject, readers).await
    }
}

#[derive(Clone)]
pub(crate) struct BoundIssueVisibility {
    pub(crate) slot: DocumentsHandlesSlot,
    pub(crate) roles: Arc<dyn NotificationAudienceResolver>,
}

impl std::fmt::Debug for BoundIssueVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundIssueVisibility")
            .field("roles", &self.roles.name())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ResourceVisibility for BoundIssueVisibility {
    fn kinds(&self) -> &'static [&'static str] {
        &[ISSUE_SUBJECT]
    }

    async fn visible(
        &self,
        subject: &SubjectRef,
        reader: &UserId,
    ) -> Result<Visibility, VisibilityError> {
        let handles = bound(&self.slot).map_err(|error| VisibilityError::Other {
            kind: ISSUE_SUBJECT.to_owned(),
            source: Box::new(error),
        })?;
        IssueVisibility::new(Arc::clone(&handles.issues), Arc::clone(&self.roles))
            .visible(subject, reader)
            .await
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BoundSourceAccess(pub(crate) DocumentsHandlesSlot);

#[async_trait]
impl SourceAccess for BoundSourceAccess {
    async fn readable(
        &self,
        subject: &SubjectRef,
        reader: &UserId,
        actor: &RequestActor,
    ) -> Result<SourceVerdict, SourceAccessError> {
        let handles = bound(&self.0).map_err(|error| SourceAccessError::Other {
            kind: DOCUMENT_SUBJECT_KIND.to_owned(),
            source: Box::new(error),
        })?;
        DocumentSourceAccess::new(
            Arc::clone(&handles.access),
            Arc::clone(&handles.repositories),
        )
        .readable(subject, reader, actor)
        .await
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BoundSourceContent(pub(crate) DocumentsHandlesSlot);

#[async_trait]
impl SourceContent for BoundSourceContent {
    fn name(&self) -> &'static str {
        "documents"
    }

    fn handles(&self, source: &FactSource) -> bool {
        matches!(
            source,
            FactSource::Document { .. } | FactSource::Comment { .. }
        )
    }

    async fn fetch(&self, source: &FactSource) -> Result<Option<SourceText>, SourceContentError> {
        let handles = bound(&self.0).map_err(|error| SourceContentError::Other {
            source_kind: "document".to_owned(),
            source: Box::new(error),
        })?;
        DocumentSourceContent::new(handles.service.clone(), Arc::clone(&handles.repositories))
            .fetch(source)
            .await
    }
}
