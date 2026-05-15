
ALTER TABLE engagement_events ADD COLUMN IF NOT EXISTS event_type VARCHAR(50) NOT NULL DEFAULT 'page_exit';
CREATE INDEX IF NOT EXISTS idx_engagement_events_event_type ON engagement_events(event_type);
