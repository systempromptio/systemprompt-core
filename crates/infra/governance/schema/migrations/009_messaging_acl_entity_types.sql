-- Admit the chat-platform entity kinds into the access-control whitelist.
--
-- `EntityKind::SlackWorkspace` / `EntityKind::TeamsTenant` ("slack_workspace" /
-- "teams_tenant") back the Slack/Teams messaging surfaces. `ingest_slack_apps`
-- and `ingest_teams_apps` project each app's `authz.allowed_roles` into
-- `access_control_entities` + `access_control_rules`, but the entity-type CHECK
-- constraints predated those kinds and rejected the INSERT at runtime. Broaden
-- both constraints to match the base schema.
--
-- Idempotent: DROP IF EXISTS guards the replay path.

ALTER TABLE access_control_entities
    DROP CONSTRAINT IF EXISTS access_control_entities_entity_type_check;
ALTER TABLE access_control_entities
    ADD CONSTRAINT access_control_entities_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill','hook','slack_workspace','teams_tenant'));

ALTER TABLE access_control_rules
    DROP CONSTRAINT IF EXISTS access_control_rules_entity_type_check;
ALTER TABLE access_control_rules
    ADD CONSTRAINT access_control_rules_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill','hook','slack_workspace','teams_tenant'));
