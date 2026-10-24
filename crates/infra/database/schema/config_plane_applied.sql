-- One row per config plane that an explicit apply (`config apply`,
-- `/admin/sync`) has recorded, whether the tree declared the plane or not.
--
-- A runtime reader whose plane table is empty reads this row to tell "the
-- operator declared nothing" from "nobody ran the apply", and fails closed on
-- the second.
CREATE TABLE IF NOT EXISTS config_plane_applied (
    plane TEXT PRIMARY KEY CHECK (length(plane) BETWEEN 1 AND 64),
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
