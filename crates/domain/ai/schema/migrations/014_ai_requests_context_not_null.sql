UPDATE ai_requests
SET context_id = '00000000-0000-0000-0000-4c4547414359'
WHERE context_id IS NULL OR context_id = '';

ALTER TABLE ai_requests ALTER COLUMN context_id SET NOT NULL;
