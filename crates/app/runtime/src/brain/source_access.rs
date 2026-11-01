//! [`DocumentSourceAccess`]: the core [`SourceAccess`] for document
//! sources.
//!
//! A fact sourced from a document or one of its comments is readable when
//! the reader holds any level on the live document, the same rule
//! [`DocumentVisibility`](crate::notifications::DocumentVisibility) applies
//! to notifications. A person then reads it; an agent reads it only when
//! the document's agent cap for that agent, acting for the reader, is above
//! `none`, else it is `AgentCapped`. Any other subject kind is hidden, and a
//! failed lookup is an error, which recall treats as withheld.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::brain::{SourceAccess, SourceAccessError, SourceVerdict};
use systemprompt_files::documents::agents::{AgentActor, AgentCap, AgentChannel};
use systemprompt_files::documents::models::SubjectPairs;
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_files::documents::{DocumentAccessService, DocumentsError, Granted};
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_models::auth::{AuthenticatedUser, RequestActor};
use systemprompt_models::brain::DOCUMENT_SUBJECT_KIND;
use systemprompt_models::notifications::SubjectRef;

/// Decides document-sourced fact readability for a reader and an actor.
#[derive(Debug, Clone)]
pub struct DocumentSourceAccess {
    access: Arc<DocumentAccessService>,
    repos: Arc<DocumentRepositories>,
}

impl DocumentSourceAccess {
    #[must_use]
    pub const fn new(access: Arc<DocumentAccessService>, repos: Arc<DocumentRepositories>) -> Self {
        Self { access, repos }
    }
}

fn failed(error: DocumentsError) -> SourceAccessError {
    SourceAccessError::Other {
        kind: DOCUMENT_SUBJECT_KIND.to_owned(),
        source: Box::new(error),
    }
}

#[async_trait]
impl SourceAccess for DocumentSourceAccess {
    async fn readable(
        &self,
        subject: &SubjectRef,
        reader: &UserId,
        actor: &RequestActor,
    ) -> Result<SourceVerdict, SourceAccessError> {
        if subject.kind != DOCUMENT_SUBJECT_KIND {
            return Ok(SourceVerdict::Hidden);
        }
        let document_id = DocumentId::new(subject.id.as_str());
        let Some(document) = self.repos.documents.find(&document_id).await? else {
            return Ok(SourceVerdict::Hidden);
        };
        let Some(level) = self
            .access
            .level_of(&document, reader)
            .await
            .map_err(failed)?
        else {
            return Ok(SourceVerdict::Hidden);
        };
        let RequestActor::Agent { name, attended } = actor else {
            return Ok(SourceVerdict::Readable);
        };
        let who = AuthenticatedUser::new(reader.clone(), String::new(), String::new(), vec![]);
        let granted = Granted {
            level,
            pairs: SubjectPairs::default(),
            document: Some(document),
            folder: None,
            agent_cap: None,
        };
        let agent = AgentActor {
            name: name.clone(),
            attended: *attended,
            channel: AgentChannel::Direct,
        };
        let effective = self
            .access
            .agent_cap_for(&who, &granted, &agent)
            .await
            .map_err(failed)?;
        Ok(if effective.cap == AgentCap::None {
            SourceVerdict::AgentCapped
        } else {
            SourceVerdict::Readable
        })
    }
}
