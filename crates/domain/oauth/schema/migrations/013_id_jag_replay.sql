-- Replay-prevention store for Enterprise-Managed Authorization (EMA) ID-JAGs.
--
-- An ID-JAG (draft-ietf-oauth-identity-assertion-authz-grant) is a single-use,
-- audience-bound assertion. The resource server records each `jti` the first
-- time it is presented and rejects any subsequent presentation. The store is
-- authoritative across API instances (a per-process cache would miss
-- cross-instance replay), so consumption is an atomic INSERT ... ON CONFLICT.
--
-- `expires_at` carries the ID-JAG's own `exp` so cleanup can drop rows once a
-- replay is no longer possible (the assertion has expired anyway).
CREATE TABLE IF NOT EXISTS id_jag_replay (
    jti        TEXT        PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL,
    seen_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_id_jag_replay_expires_at ON id_jag_replay (expires_at);
