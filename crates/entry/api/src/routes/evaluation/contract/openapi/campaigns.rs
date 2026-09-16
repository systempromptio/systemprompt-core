//! Full campaign policy, dispatch, guided holdout and reviewed source
//! lifecycle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::builder::Document;
use crate::routes::evaluation::collections::{Cursor, Page};
use crate::routes::evaluation::{campaign_completion as holdout, campaigns as api};
use systemprompt_evaluation::campaigns::holdout::HoldoutProposal;
use systemprompt_evaluation::campaigns::report::CampaignReport;
use systemprompt_evaluation::campaigns::repository::{CampaignRecord, CampaignTransition};
use systemprompt_evaluation::repository::experiments::CampaignExperiment;
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId};
use systemprompt_runtime::optimization::SourceAcceptance;
use systemprompt_runtime::optimization::holdout::{ConfirmHoldout, HoldoutReview, PrepareHoldout};
pub(super) fn register(d: &mut Document) {
    d.add::<systemprompt_evaluation::repository::experiments::SuggestionRequest,systemprompt_evaluation::campaigns::suggestions::RetainedSuggestion>("/evaluation-suggestions","post",201,false);
    d.add::<(), systemprompt_evaluation::campaigns::suggestions::RetainedSuggestion>(
        "/evaluation-suggestions/{id}",
        "get",
        200,
        false,
    );
    d.add::<(), systemprompt_evaluation::repository::experiments::ExecutionApproval>(
        "/evaluation-approvals/{id}",
        "get",
        200,
        false,
    );
    d.add::<crate::routes::evaluation::lifecycle::DecideApproval,crate::routes::evaluation::operations::OperationResponse<systemprompt_evaluation::repository::experiments::ExecutionApproval>>("/evaluation-approvals/{id}/decisions","post",200,false);
    d.idempotent("/evaluation-approvals/{id}/decisions");

    d.add::<systemprompt_marketplace::managed::PublicationRequest,systemprompt_marketplace::managed::PublicationDecision>("/publications","post",200,false);
    d.add::<(), Page<systemprompt_marketplace::managed::PublicationHistoryEntry>>(
        "/resources/{id}/publications",
        "get",
        200,
        false,
    );
    d.query::<crate::routes::evaluation::publications::HistoryQuery>(
        "/resources/{id}/publications",
        "get",
    );
    d.add::<(), api::Page>("/campaigns", "get", 200, false);
    d.query::<api::Cursor>("/campaigns", "get");
    d.add::<api::Create, EvalCampaignId>("/campaigns", "post", 201, false);
    d.add::<(), CampaignRecord>("/campaigns/{id}", "get", 200, false);
    d.add::<CampaignTransition, ()>("/campaigns/{id}/transitions", "post", 204, false);
    d.add::<(), Page<EvalExperimentId>>("/campaigns/{id}/experiments", "get", 200, false);
    d.query::<Cursor>("/campaigns/{id}/experiments", "get");
    d.add::<api::Attach, ()>("/campaigns/{id}/experiments", "post", 204, false);
    d.add::<(), CampaignReport>(
        "/campaigns/{id}/experiments/{experiment}/report",
        "get",
        200,
        false,
    );
    d.add::<CampaignExperiment, EvalExperimentId>("/campaign-runs", "post", 202, false);
    d.add::<SourceAcceptance,systemprompt_marketplace::managed::evaluation::EvaluationAttestation>("/source-changes","post",201,false);
    d.add::<(), holdout::DiagnosticPage>("/campaign-diagnostics", "get", 200, false);
    d.query::<holdout::DiagnosticQuery>("/campaign-diagnostics", "get");
    d.add::<PrepareHoldout, HoldoutReview>("/campaigns/{id}/holdout-proposals", "post", 200, false);
    d.add::<(), HoldoutProposal>(
        "/campaigns/{id}/holdout-proposals/{proposal}",
        "get",
        200,
        false,
    );
    d.add::<ConfirmHoldout, HoldoutProposal>(
        "/campaigns/{id}/holdout-proposals/{proposal}/confirm",
        "post",
        200,
        false,
    );
}
