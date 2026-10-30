CREATE TABLE IF NOT EXISTS brain_fact_revisions (
    id TEXT PRIMARY KEY,
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    revision INT NOT NULL CHECK (revision > 0),
    change TEXT NOT NULL CHECK (change IN (
        'created', 'revised', 'merged', 'fields_edited', 'confirmed', 'disputed',
        'dispute_resolved', 'staled', 'superseded', 'retracted', 'pinned', 'unpinned',
        'review_due'
    )),
    actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK (status IN (
        'candidate', 'proposed', 'confirmed', 'disputed', 'stale', 'superseded', 'retired'
    )),
    statement TEXT NOT NULL,
    fields JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (fact_id, revision)
);
