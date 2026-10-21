-- Release stamps (schema/release_stamps.sql): the release that last completed
-- a once-per-release run, read and written by ReleaseStampRepository. The
-- table is the base schema's text.
CREATE TABLE IF NOT EXISTS release_stamps (
    name TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    stamped_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
