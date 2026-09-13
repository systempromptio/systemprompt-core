-- Forward owner constraints for experiment relations created before the
-- supervised evaluator schema.
CREATE OR REPLACE FUNCTION enforce_eval_owner_scope() RETURNS trigger AS $$
DECLARE expected_owner text; related_owner text;
BEGIN
    IF TG_TABLE_NAME = 'eval_executions' THEN
        SELECT owner_id INTO expected_owner FROM eval_experiments WHERE id=NEW.experiment_id;
        SELECT owner_id INTO related_owner FROM eval_resource_revisions WHERE id=NEW.case_revision_id;
    ELSIF TG_TABLE_NAME = 'eval_session_bindings' THEN
        SELECT e.owner_id INTO expected_owner FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=NEW.execution_id;
        related_owner := NEW.owner_id;
    ELSIF TG_TABLE_NAME = 'eval_execution_capabilities' THEN
        SELECT e.owner_id INTO expected_owner FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=NEW.execution_id;
        SELECT w.owner_id INTO related_owner FROM eval_workers w JOIN user_sessions s ON s.user_id=w.owner_id WHERE w.id=NEW.worker_id AND s.session_id=NEW.session_id;
    ELSE
        SELECT e.owner_id INTO expected_owner FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=NEW.execution_id;
        SELECT a.owner_id INTO related_owner FROM eval_budget_reservations r JOIN eval_budget_accounts a ON a.id=r.account_id JOIN ai_requests q ON q.user_id=a.owner_id WHERE r.id=NEW.reservation_id AND q.id=NEW.request_id;
    END IF;
    IF expected_owner IS NULL OR related_owner IS NULL OR expected_owner <> related_owner THEN RAISE EXCEPTION 'evaluation ownership conflict' USING ERRCODE='23514'; END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS eval_execution_owner_scope ON eval_executions;
CREATE TRIGGER eval_execution_owner_scope BEFORE INSERT OR UPDATE OF experiment_id,case_revision_id ON eval_executions FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
DROP TRIGGER IF EXISTS eval_session_owner_scope ON eval_session_bindings;
CREATE TRIGGER eval_session_owner_scope BEFORE INSERT OR UPDATE ON eval_session_bindings FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
DROP TRIGGER IF EXISTS eval_capability_owner_scope ON eval_execution_capabilities;
CREATE TRIGGER eval_capability_owner_scope BEFORE INSERT OR UPDATE ON eval_execution_capabilities FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
DROP TRIGGER IF EXISTS eval_request_owner_scope ON eval_request_reservations;
CREATE TRIGGER eval_request_owner_scope BEFORE INSERT OR UPDATE ON eval_request_reservations FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();

CREATE OR REPLACE FUNCTION enforce_eval_lifecycle_owner() RETURNS trigger AS $$
DECLARE experiment_owner text; related_owner text;
BEGIN
    IF TG_TABLE_NAME = 'eval_execution_approvals' THEN
        SELECT e.owner_id INTO experiment_owner FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=NEW.execution_id;
        related_owner := NEW.owner_id;
    ELSIF TG_TABLE_NAME = 'eval_suggestions' THEN
        SELECT owner_id INTO experiment_owner FROM eval_experiments WHERE id=NEW.experiment_id;
        SELECT a.owner_id INTO related_owner FROM eval_budget_reservations r JOIN eval_budget_accounts a ON a.id=r.account_id WHERE r.id=NEW.reservation_id AND a.owner_id=NEW.owner_id;
    ELSE
        SELECT e.owner_id INTO experiment_owner FROM eval_experiments e JOIN eval_resource_revisions c ON c.id=NEW.case_revision_id AND c.owner_id=e.owner_id WHERE e.id=NEW.experiment_id;
        related_owner := NEW.owner_id;
    END IF;
    IF experiment_owner IS NULL OR related_owner IS NULL OR experiment_owner<>related_owner THEN RAISE EXCEPTION 'evaluation lifecycle ownership conflict' USING ERRCODE='23514'; END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS eval_approval_owner_scope ON eval_execution_approvals;
CREATE TRIGGER eval_approval_owner_scope BEFORE INSERT OR UPDATE ON eval_execution_approvals FOR EACH ROW EXECUTE FUNCTION enforce_eval_lifecycle_owner();
DROP TRIGGER IF EXISTS eval_suggestion_owner_scope ON eval_suggestions;
CREATE TRIGGER eval_suggestion_owner_scope BEFORE INSERT OR UPDATE ON eval_suggestions FOR EACH ROW EXECUTE FUNCTION enforce_eval_lifecycle_owner();
DROP TRIGGER IF EXISTS eval_holdout_owner_scope ON eval_holdout_consumption;
CREATE TRIGGER eval_holdout_owner_scope BEFORE INSERT OR UPDATE ON eval_holdout_consumption FOR EACH ROW EXECUTE FUNCTION enforce_eval_lifecycle_owner();
