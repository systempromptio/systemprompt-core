-- Gateway policy writes carry their change context and bump the config
-- version, which notifies `config:gateway_policies` to every node.
CREATE OR REPLACE TRIGGER ai_gateway_policies_stamp_change
BEFORE INSERT OR UPDATE ON ai_gateway_policies
FOR EACH ROW EXECUTE FUNCTION sp_stamp_change();
CREATE OR REPLACE TRIGGER ai_gateway_policies_config_version
AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON ai_gateway_policies
FOR EACH STATEMENT EXECUTE FUNCTION sp_bump_config_version('gateway_policies');
