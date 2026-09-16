//! Managed sources, immutable revision bundles, retained verification and
//! human publication review.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::builder::Document;
use crate::routes::managed::collections::Page;
use crate::routes::managed::operation_handlers::CaptureSource;
use crate::routes::managed::operations::{OperationResponse, OperationResult};
use crate::routes::managed::resources as api;
use systemprompt_identifiers::ManagedSourceId;
use systemprompt_marketplace::managed::{
    ImportedSkills, PublicationDecision, PublicationHistoryEntry, PublicationRequest,
    RevisionBundle, SourceSpec,
};
use systemprompt_models::feedback::verification::{
    DependencyVerificationManifest, DependencyVerificationRequest,
};
pub(super) fn register(d: &mut Document) {
    d.add::<api::CreateSource, ManagedSourceId>("/sources", "post", 201, false);
    d.add::<(), SourceSpec>("/sources/{id}", "get", 200, false);
    d.add::<api::VerificationSourceBinding, ()>(
        "/sources/{id}/verification-bindings",
        "post",
        204,
        false,
    );
    d.add::<CaptureSource, OperationResponse<ImportedSkills>>(
        "/sources/{id}/captures",
        "post",
        200,
        false,
    );
    d.idempotent("/sources/{id}/captures");
    d.add::<(), RevisionBundle>("/revisions/{id}/bundle", "get", 200, false);
    d.add::<DependencyVerificationRequest, OperationResponse<DependencyVerificationManifest>>(
        "/source-verifications",
        "post",
        200,
        false,
    );
    d.idempotent("/source-verifications");
    d.add::<(), DependencyVerificationManifest>("/source-verifications/{id}", "get", 200, false);
    d.add::<(), OperationResponse<OperationResult>>("/operations/{id}", "get", 200, false);
    d.add::<PublicationRequest, PublicationDecision>("/publications", "post", 200, false);
    d.add::<(), Page<PublicationHistoryEntry>>("/resources/{id}/publications", "get", 200, false);
    d.query::<crate::routes::managed::publications::HistoryQuery>(
        "/resources/{id}/publications",
        "get",
    );
}
