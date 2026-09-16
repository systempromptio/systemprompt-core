//! Exact executable capabilities, immutable inputs and retained verification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::builder::Document;
use crate::routes::evaluation::collections::{Cursor, Page};
use crate::routes::evaluation::operation_handlers::CaptureSource;
use crate::routes::evaluation::operations::{OperationResponse, OperationResult};
use crate::routes::evaluation::optimization_resources as api;
use systemprompt_evaluation::capabilities::EvaluatorCapability;
use systemprompt_evaluation::experiments::records::{
    BudgetRecord, ExecutionRecord, ExperimentRecord,
};
use systemprompt_evaluation::experiments::resources::ResourceContent;
use systemprompt_identifiers::{EvalBudgetId, EvalRevisionId, ManagedSourceId};
use systemprompt_marketplace::managed::{ImportedSkills, RevisionBundle, SourceSpec};
use systemprompt_models::feedback::verification::{
    DependencyVerificationManifest, DependencyVerificationRequest,
};
pub(super) fn register(d: &mut Document) {
    d.add::<(), Page<ExperimentRecord>>("/experiments", "get", 200, false);
    d.query::<Cursor>("/experiments", "get");
    d.add::<(), crate::routes::evaluation::execution_pages::ExperimentPage>(
        "/experiments/{id}",
        "get",
        200,
        false,
    );
    d.add::<(), Page<ExecutionRecord>>("/experiments/{id}/executions", "get", 200, false);
    d.query::<Cursor>("/experiments/{id}/executions", "get");
    d.add::<(), ()>("/experiments/{id}/cancellation", "post", 204, false);
    d.add::<api::CreateBudget, EvalBudgetId>("/budgets", "post", 201, false);
    d.add::<(), BudgetRecord>("/budgets/{id}", "get", 200, false);
    d.add::<api::CreateEvaluationRevision, EvalRevisionId>(
        "/evaluation-revisions",
        "post",
        201,
        false,
    );
    d.add::<(), ResourceContent>("/evaluation-revisions/{id}", "get", 200, false);
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
    d.add::<(), String>("/revisions/{id}/workspace", "post", 200, false);
    d.add::<DependencyVerificationRequest, OperationResponse<DependencyVerificationManifest>>(
        "/source-verifications",
        "post",
        200,
        false,
    );
    d.idempotent("/source-verifications");
    d.add::<(), DependencyVerificationManifest>("/source-verifications/{id}", "get", 200, false);
    d.add::<(), Page<EvaluatorCapability>>("/evaluator-capabilities", "get", 200, false);
    d.query::<Cursor>("/evaluator-capabilities", "get");
    d.add::<(), OperationResponse<OperationResult>>("/operations/{id}", "get", 200, false);
}
