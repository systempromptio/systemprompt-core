ALTER TABLE engagement_events
ADD COLUMN content_id TEXT REFERENCES markdown_content(id);

CREATE INDEX idx_engagement_events_content ON engagement_events(content_id);
