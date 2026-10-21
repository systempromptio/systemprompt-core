-- Reject UPDATE on governance_decisions while the trigger is enabled.
-- DELETE remains available for retention and erasure under database grants.
-- Owners can disable the trigger; protect owner credentials separately.

CREATE OR REPLACE FUNCTION governance_decisions_deny_update()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'governance_decisions is append-only: UPDATE is refused'
        USING ERRCODE = 'restrict_violation';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER governance_decisions_append_only
    BEFORE UPDATE ON governance_decisions
    FOR EACH ROW
    EXECUTE FUNCTION governance_decisions_deny_update();
