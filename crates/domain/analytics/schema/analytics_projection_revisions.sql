CREATE TABLE IF NOT EXISTS analytics_projection_revisions (
    source TEXT NOT NULL,
    entity_key TEXT NOT NULL,
    revision BIGINT NOT NULL,
    PRIMARY KEY (source, entity_key)
);
