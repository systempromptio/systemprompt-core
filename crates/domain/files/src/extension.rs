//! [`FilesExtension`]: contributes the files capability and the document
//! store and issue reports it hosts.
//!
//! The files contributions (`contribute_files`) are the `files` and
//! `content_files` schemas, the `files` user-purge row, the
//! `file_ingestion` job and the provided `Arc<FileRepository>`, which the
//! `files-http` member's commands resolve. The document and issue
//! contributions (`contribute_documents_and_issues`) are the document store
//! schemas, the `report_document_revisions` and `report_document_sharing`
//! views, the document and issue jobs, the document-actor job run scope and
//! the user-purge rows of the document store and issue report tables. The
//! migrations are one chain for the crate.
//!
//! It opens the three files slots ([`BlobHolderSlot`], [`AgentAccessSlot`],
//! [`DocumentsSlot`]) and fills them with the issue attachment holder, the
//! core codecs and the core template provider; it contributes no agent
//! access policy, so the default applies unless one other member does.
//!
//! It provides its loaded configuration (`Arc<FilesConfig>`) and the open
//! view the API host reads (`Arc<FilesConfigView>`), contributes the files
//! startup validator, and declares the document store late cell
//! ([`DocumentsHandlesSlot`]) the kernel seeds and binds. It provides the
//! core defaults other domains' slots append, each a [`CoreSeam`] that reads
//! that cell: the
//! document and issue visibilities the notifications slot appends, and the
//! document source access and source content the brain slot reads. A seam
//! called before the kernel binds the cell fails with `DocumentsUnbound`. The
//! issue visibility asks the user-role audience resolver the `users` member
//! provides for triage-role membership, so a composition without `users`
//! refuses this member's provide.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::brain::{SourceAccess, SourceContent};
use systemprompt_extension::prelude::*;
use systemprompt_extension::{NotificationAudienceResolver, ResourceVisibility};
use systemprompt_provider_contracts::documents::{DocumentCodec, DocumentTemplateProvider};
use systemprompt_traits::CoreSeam;
use systemprompt_traits::core_seam::{self, RoleAudience};

use crate::contract::{AgentAccessSlot, BlobHolderSlot, DocumentsSlot};
use crate::document_schemas::DOCUMENT_SCHEMAS;
use crate::documents::DocumentsHandlesSlot;
use crate::documents::codecs::core_codecs;
use crate::documents::handles::{
    BoundDocumentVisibility, BoundIssueVisibility, BoundSourceAccess, BoundSourceContent,
};
use crate::documents::jobs::{
    DocumentAgentReadRetentionJob, DocumentBlobGcJob, DocumentCollabUpdateSweepJob,
    DocumentMediaBackfillJob, DocumentNodeIdsBackfillJob, DocumentPresenceSweepJob,
    DocumentTrashPurgeJob,
};
use crate::documents::repository::DOCUMENT_USER_PURGES;
use crate::documents::templates::CoreTemplateProvider;
use crate::issues::IssueRetentionJob;
use crate::issues::repository::{ISSUE_ATTACHMENTS_HOLDER, ISSUE_USER_PURGES};
use crate::jobs::{DocumentActorScope, FileIngestionJob};
use crate::repository::FileRepository;
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
        c.migrations(extension_migrations!())
            .late_cell::<DocumentsHandlesSlot>()
            .domain_config(crate::FilesConfigValidator::boxed)
            .provide(crate::provides::files_config)
            .provide(crate::provides::files_config_view);
        contribute_files(c);
        contribute_documents_and_issues(c);
        c.owner_reassignment(OwnerReassignment {
            table: "files",
            column: "user_id",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "documents",
            column: "owner_user_id",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "documents",
            column: "created_by",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "document_folders",
            column: "owner_user_id",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "document_folders",
            column: "created_by",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "document_revisions",
            column: "author_user_id",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "document_grants",
            column: "granted_by",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "documents",
            column: "trashed_by",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "document_comments",
            column: "author_user_id",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "document_comments",
            column: "resolved_by",
            action: ReassignAction::UPDATE,
        });
        Ok(())
    }
}

fn contribute_files(c: &mut Contributions<'_>) {
    c.schema(
        SchemaDefinition::new("files", include_str!("../schema/files.sql")).with_required_columns(
            vec![
                "id".into(),
                "path".into(),
                "mime_type".into(),
                "created_at".into(),
            ],
        ),
    )
    .schema(
        SchemaDefinition::new("content_files", include_str!("../schema/content_files.sql"))
            .with_required_columns(vec!["id".into(), "content_id".into(), "file_id".into()]),
    )
    .user_purge(UserPurgeTable {
        table: "files",
        column: "user_id",
        predicate: None,
    })
    .job(FileIngestionJob)
    .provide(
        |r: &Resolved<'_>| -> Result<Arc<FileRepository>, ProvideError> {
            Ok(Arc::new(FileRepository::new(&r.get::<DbPool>()?)))
        },
    );
}

fn contribute_documents_and_issues(c: &mut Contributions<'_>) {
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
    for purge in DOCUMENT_USER_PURGES.iter().chain(ISSUE_USER_PURGES) {
        c.user_purge(*purge);
    }
    c.job(IssueRetentionJob)
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
