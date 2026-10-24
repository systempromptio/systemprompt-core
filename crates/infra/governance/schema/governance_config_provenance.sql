-- Governance-plane writes carry their change context and bump the config
-- version, which notifies `config:governance` to every node.
CREATE OR REPLACE TRIGGER governance_config_stamp_change
BEFORE INSERT OR UPDATE ON governance_config
FOR EACH ROW EXECUTE FUNCTION sp_stamp_change();
CREATE OR REPLACE TRIGGER governance_config_config_version
AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON governance_config
FOR EACH STATEMENT EXECUTE FUNCTION sp_bump_config_version('governance');
