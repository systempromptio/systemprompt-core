UPDATE engagement_events ee
SET content_id = mc.id
FROM markdown_content mc
WHERE ee.page_url LIKE '%/' || mc.slug
  AND ee.content_id IS NULL;
