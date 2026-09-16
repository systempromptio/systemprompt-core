CREATE OR REPLACE VIEW reporting_source_markdown_content AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,slug,title,source_id FROM markdown_content) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON markdown_content
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('markdown_content', 'id', 'id,slug,title,source_id');


