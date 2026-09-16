-- The evaluation engine (campaigns, experiments, executions, budgets, workers,
-- holdouts, suggestions, approvals) is removed. The product observes and
-- compares skill versions through the managed marketplace; it does not run
-- experiments. Nothing was ever admitted to the engine, so the tables are
-- dropped rather than exported. CASCADE takes every dependent trigger, view
-- and foreign key with them, including hooks other extensions attached.
DROP TABLE IF EXISTS eval_approved_operation_receipts CASCADE;
DROP TABLE IF EXISTS eval_budget_accounts CASCADE;
DROP TABLE IF EXISTS eval_budget_reservations CASCADE;
DROP TABLE IF EXISTS eval_campaign_diagnostics CASCADE;
DROP TABLE IF EXISTS eval_campaign_events CASCADE;
DROP TABLE IF EXISTS eval_campaign_experiments CASCADE;
DROP TABLE IF EXISTS eval_campaign_holdout_proposals CASCADE;
DROP TABLE IF EXISTS eval_campaign_source_changes CASCADE;
DROP TABLE IF EXISTS eval_campaigns CASCADE;
DROP TABLE IF EXISTS eval_cases CASCADE;
DROP TABLE IF EXISTS eval_execution_approvals CASCADE;
DROP TABLE IF EXISTS eval_execution_artifacts CASCADE;
DROP TABLE IF EXISTS eval_execution_capabilities CASCADE;
DROP TABLE IF EXISTS eval_execution_cleanup CASCADE;
DROP TABLE IF EXISTS eval_execution_events CASCADE;
DROP TABLE IF EXISTS eval_execution_evidence CASCADE;
DROP TABLE IF EXISTS eval_execution_measurements CASCADE;
DROP TABLE IF EXISTS eval_executions CASCADE;
DROP TABLE IF EXISTS eval_experiments CASCADE;
DROP TABLE IF EXISTS eval_fixture_payloads CASCADE;
DROP TABLE IF EXISTS eval_fixture_test_records CASCADE;
DROP TABLE IF EXISTS eval_holdout_consumption CASCADE;
DROP TABLE IF EXISTS eval_holdout_content_consumption CASCADE;
DROP TABLE IF EXISTS eval_judge_calls CASCADE;
DROP TABLE IF EXISTS eval_managed_workspace_assets CASCADE;
DROP TABLE IF EXISTS eval_managed_workspace_projections CASCADE;
DROP TABLE IF EXISTS eval_pairs CASCADE;
DROP TABLE IF EXISTS eval_request_reservations CASCADE;
DROP TABLE IF EXISTS eval_resource_revisions CASCADE;
DROP TABLE IF EXISTS eval_results CASCADE;
DROP TABLE IF EXISTS eval_rubrics CASCADE;
DROP TABLE IF EXISTS eval_runs CASCADE;
DROP TABLE IF EXISTS eval_session_bindings CASCADE;
DROP TABLE IF EXISTS eval_suggestions CASCADE;
DROP TABLE IF EXISTS eval_workers CASCADE;

DROP FUNCTION IF EXISTS enforce_eval_lifecycle_owner() CASCADE;
DROP FUNCTION IF EXISTS enforce_eval_owner_scope() CASCADE;
DROP FUNCTION IF EXISTS eval_ensure_unique_key() CASCADE;
DROP FUNCTION IF EXISTS protect_campaign_holdout_proposal() CASCADE;
DROP FUNCTION IF EXISTS reject_eval_managed_workspace_change() CASCADE;
DROP FUNCTION IF EXISTS reject_eval_operation_receipt_change() CASCADE;

-- Publication review no longer binds to an attested experiment.
DROP TABLE IF EXISTS managed_evaluation_attestations CASCADE;
ALTER TABLE managed_publication_reviews DROP COLUMN IF EXISTS experiment_id;

-- Invocation traffic is production or fixture; the engine's classes had no
-- production writer.
ALTER TABLE managed_invocation_attributions
    DROP CONSTRAINT IF EXISTS managed_invocation_attributions_traffic_class_check;
ALTER TABLE managed_invocation_attributions
    ADD CONSTRAINT managed_invocation_attributions_traffic_class_check
    CHECK (traffic_class IN ('production','fixture'));

-- The migration runner is keyed by extension; with the extension gone its
-- rows would otherwise be orphaned forever.
DELETE FROM extension_migrations WHERE extension_id = 'evaluation';
