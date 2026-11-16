//! [`FilesExtension`]: contributes the `files`, `content_files` and document
//! store schemas, the `report_document_revisions` and
//! `report_document_sharing` views, the migrations, the files jobs and the
//! document-actor job run scope, and the user-purge rows of the document store
//! and issue report tables.
//!
//! It opens the three files slots ([`BlobHolderSlot`], [`AgentAccessSlot`],
//! [`DocumentsSlot`]) and fills them with the issue attachment holder, the
//! core codecs and the core template provider; it contributes no agent
//! access policy, so the default applies unless one other member does.
//!
//! It provides the core defaults other domains' slots append, each a
//! [`CoreSeam`] that reads the [`DocumentsHandlesSlot`] the host seeds and
//! binds once it has built the document store from the resolved state: the
//! document and issue visibilities the notifications slot appends, and the
//! document source access and source content the brain slot reads. A seam
//! called before the host binds the slot fails with `DocumentsUnbound`. The
//! issue visibility asks the user-role audience resolver the `users` member
//! provides for triage-role membership, so a composition without `users`
//! refuses this member's provide.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::brain::{SourceAccess, SourceContent};
use systemprompt_extension::prelude::*;
use systemprompt_extension::{NotificationAudienceResolver, ResourceVisibility};
use systemprompt_provider_contracts::documents::{DocumentCodec, DocumentTemplateProvider};
use systemprompt_traits::CoreSeam;
use systemprompt_traits::core_seam::{self, RoleAudience};

use crate::contract::{AgentAccessSlot, BlobHolderSlot, DocumentsSlot};
use crate::document_schemas::DOCUMENT_SCHEMAS;
use crate::documents::codecs::core_codecs;
use crate::documents::jobs::{
    DocumentAgentReadRetentionJob, DocumentBlobGcJob, DocumentCollabUpdateSweepJob,
    DocumentMediaBackfillJob, DocumentNodeIdsBackfillJob, DocumentPresenceSweepJob,
    DocumentTrashPurgeJob,
};
use crate::documents::DocumentsHandlesSlot;
use crate::documents::handles::{
    BoundDocumentVisibility, BoundIssueVisibility, BoundSourceAccess, BoundSourceContent,
};
use crate::documents::repository::DOCUMENT_USER_PURGES;
use crate::documents::templates::CoreTemplateProvider;
use crate::issues::IssueRetentionJob;
use crate::issues::repository::{ISSUE_ATTACHMENTS_HOLDER, ISSUE_USER_PURGES};
use crate::jobs::{DocumentActorScope, FileIngestionJob};
use crate::uploads::DocumentUploadExpiryJob;

#[derive(Debug, Clone, Copy, Default)]
pub struct FilesExtension;

impl Extension for FilesExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "files",
            name: "Files",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.schema(
            SchemaDefinition::new("files", include_str!("../schema/files.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "path".into(),
                    "mime_type".into(),
                    "created_at".into(),
                ]),
        )
        .schema(
            SchemaDefinition::new("content_files", include_str!("../schema/content_files.sql"))
                .with_required_columns(vec!["id".into(), "content_id".into(), "file_id".into()]),
        );
        for (table, sql, columns) in DOCUMENT_SCHEMAS {
            c.schema(
                SchemaDefinition::new(*table, *sql)
                    .with_required_columns(columns.iter().map(|c| (*c).into()).collect()),
            );
        }
        for view in [
            include_str!("../schema/report_document_revisions.sql"),
            include_str!("../schema/report_document_sharing.sql"),
        ] {
            c.schema(SchemaDefinition::sql_only(view));
        }
        c.migrations(extension_migrations!());
        c.user_purge(UserPurgeTable {
            table: "files",
            column: "user_id",
            predicate: None,
        });
        for purge in DOCUMENT_USER_PURGES.iter().chain(ISSUE_USER_PURGES) {
            c.user_purge(*purge);
        }
        c.job(FileIngestionJob)
            .job(IssueRetentionJob)
            .job(DocumentUploadExpiryJob)
            .job(DocumentAgentReadRetentionJob)
            .job(DocumentBlobGcJob)
            .job(DocumentCollabUpdateSweepJob)
            .job(DocumentMediaBackfillJob)
            .job(DocumentNodeIdsBackfillJob)
            .job(DocumentPresenceSweepJob)
            .job(DocumentTrashPurgeJob)
            .job_run_scope(Arc::new(DocumentActorScope));
        #[cfg(feature = "collab")]
        c.job(crate::documents::jobs::DocumentCollabReaperJob);
        provide_core_seams(c);
        c.slot::<BlobHolderSlot>().holder(ISSUE_ATTACHMENTS_HOLDER);
        c.slot::<AgentAccessSlot>();
        let documents = c.slot::<DocumentsSlot>();
        for codec in core_codecs() {
            documents.codec(
                move |_: &Resolved<'_>| -> Result<Arc<dyn DocumentCodec>, FactoryError> {
                    Ok(Arc::clone(&codec))
                },
            );
        }
        documents.template_provider(
            |_: &Resolved<'_>| -> Result<Arc<dyn DocumentTemplateProvider>, FactoryError> {
                Ok(Arc::new(CoreTemplateProvider))
            },
        );
        Ok(())
    }
}

type DocumentVisibilitySeam = CoreSeam<dyn ResourceVisibility, core_seam::DocumentVisibility>;
type IssueVisibilitySeam = CoreSeam<dyn ResourceVisibility, core_seam::IssueVisibility>;
type SourceAccessSeam = CoreSeam<dyn SourceAccess, core_seam::DocumentSourceAccess>;
type SourceContentSeam = CoreSeam<dyn SourceContent, core_seam::DocumentSourceContent>;

fn provide_core_seams(c: &mut Contributions<'_>) {
    c.provide(
        |r: &Resolved<'_>| -> Result<DocumentVisibilitySeam, ProvideError> {
            let seam: Arc<dyn ResourceVisibility> =
                Arc::new(BoundDocumentVisibility(r.get::<DocumentsHandlesSlot>()?));
            Ok(CoreSeam::new(seam))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<IssueVisibilitySeam, ProvideError> {
            let roles = r.get::<CoreSeam<dyn NotificationAudienceResolver, RoleAudience>>()?;
            let seam: Arc<dyn ResourceVisibility> = Arc::new(BoundIssueVisibility {
                slot: r.get::<DocumentsHandlesSlot>()?,
                roles: roles.seam(),
            });
            Ok(CoreSeam::new(seam))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<SourceAccessSeam, ProvideError> {
            let seam: Arc<dyn SourceAccess> =
                Arc::new(BoundSourceAccess(r.get::<DocumentsHandlesSlot>()?));
            Ok(CoreSeam::new(seam))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<SourceContentSeam, ProvideError> {
            let seam: Arc<dyn SourceContent> =
                Arc::new(BoundSourceContent(r.get::<DocumentsHandlesSlot>()?));
            Ok(CoreSeam::new(seam))
        },
    );
}
