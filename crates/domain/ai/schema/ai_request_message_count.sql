-- `message_count` on ai_requests: one counter maintained per statement over
-- ai_request_messages, so a reader never counts the message table.
CREATE OR REPLACE FUNCTION sp_ai_request_message_count() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE ai_requests r SET message_count = r.message_count + c.n
        FROM (SELECT request_id, count(*)::integer AS n FROM new_rows GROUP BY request_id) c
        WHERE r.id = c.request_id;
    ELSE
        UPDATE ai_requests r SET message_count = GREATEST(r.message_count - c.n, 0)
        FROM (SELECT request_id, count(*)::integer AS n FROM old_rows GROUP BY request_id) c
        WHERE r.id = c.request_id;
    END IF;
    RETURN NULL;
END;
$$;
CREATE OR REPLACE TRIGGER message_count_insert AFTER INSERT ON ai_request_messages
REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_ai_request_message_count();
CREATE OR REPLACE TRIGGER message_count_delete AFTER DELETE ON ai_request_messages
REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_ai_request_message_count();
