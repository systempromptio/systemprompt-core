//! Evaluation subsystem identifiers (runs, cases, results, pairs, judge
//! calls, rubrics).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(EvalRunId, generate, schema);
crate::define_id!(EvalCaseId, generate, schema);
crate::define_id!(EvalResultId, generate, schema);
crate::define_id!(EvalPairId, generate, schema);
crate::define_id!(EvalJudgeCallId, generate, schema);
crate::define_id!(EvalRubricId, generate, schema);

crate::define_id!(EvalExperimentId, generate, schema);
crate::define_id!(EvalExecutionId, generate, schema);
crate::define_id!(EvalRevisionId, generate, schema);
crate::define_id!(EvalBudgetId, generate, schema);
crate::define_id!(EvalReservationId, generate, schema);

crate::define_id!(EvalWorkerId, generate, schema);
