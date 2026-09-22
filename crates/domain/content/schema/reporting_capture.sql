CREATE OR REPLACE VIEW reporting_source_markdown_content AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,slug,title,source_id FROM markdown_content) fact;
DROP TRIGGER IF EXISTS reporting_capture ON markdown_content;
CREATE OR REPLACE TRIGGER reporting_capture_insert AFTER INSERT ON markdown_content
REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('markdown_content', 'id', 'id,slug,title,source_id');
CREATE OR REPLACE TRIGGER reporting_capture_update AFTER UPDATE ON markdown_content
REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('markdown_content', 'id', 'id,slug,title,source_id');
CREATE OR REPLACE TRIGGER reporting_capture_delete AFTER DELETE ON markdown_content
REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('markdown_content', 'id', 'id,slug,title,source_id');


