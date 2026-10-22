-- Access-control writes carry their change context and bump the config
-- version, which notifies `config:access_control` to every node.
CREATE OR REPLACE TRIGGER access_control_rules_stamp_change
BEFORE INSERT OR UPDATE ON access_control_rules
FOR EACH ROW EXECUTE FUNCTION sp_stamp_change();
CREATE OR REPLACE TRIGGER access_control_entities_stamp_change
BEFORE INSERT OR UPDATE ON access_control_entities
FOR EACH ROW EXECUTE FUNCTION sp_stamp_change();
CREATE OR REPLACE TRIGGER access_control_rules_config_version
AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON access_control_rules
FOR EACH STATEMENT EXECUTE FUNCTION sp_bump_config_version('access_control');
CREATE OR REPLACE TRIGGER access_control_entities_config_version
AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON access_control_entities
FOR EACH STATEMENT EXECUTE FUNCTION sp_bump_config_version('access_control');
