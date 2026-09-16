//! Evaluation subsystem identifiers (golden cases, experiments, executions,
//! budgets, workers).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(EvalCaseId, generate, schema);
crate::define_id!(EvalCampaignId, generate, schema);

crate::define_id!(EvalExperimentId, generate, schema);
crate::define_id!(EvalExecutionId, generate, schema);
crate::define_id!(EvalRevisionId, generate, schema);
crate::define_id!(EvalBudgetId, generate, schema);
crate::define_id!(EvalReservationId, generate, schema);
crate::define_id!(EvalSuggestionId, generate, schema);
crate::define_id!(EvalApprovalId, generate, schema);
crate::define_id!(EvalHoldoutProposalId, generate, schema);

crate::define_id!(EvalWorkerId, generate, schema);
