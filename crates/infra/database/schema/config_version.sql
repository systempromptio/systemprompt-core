-- One row counting every committed write to a config plane table.
--
-- `sp_bump_config_version()` upserts it from a statement trigger on each plane
-- table, so the row exists from the first plane write without a seed. A node
-- polls `version` as the fallback when a `systemprompt_cache` notification is
-- lost: a moved number means some plane changed and its cache reloads.
CREATE TABLE IF NOT EXISTS config_version (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    version BIGINT NOT NULL CHECK (version > 0),
    plane TEXT NOT NULL CHECK (length(plane) BETWEEN 1 AND 64),
    bumped_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
