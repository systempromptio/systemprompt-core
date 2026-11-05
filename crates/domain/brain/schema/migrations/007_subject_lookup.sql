-- Recall latency (P3, wave 3): the subjects a message names are looked up
-- by the message's word n-grams, so brain_aliases gains a btree on
-- alias_norm alone (the unique key leads with subject_kind) and
-- brain_subjects an expression index on lower(subject_id). Keys only: each
-- build reads its table once (300k aliases, ~1 s).
-- @cost: rows=0 measured=1.1s triggers=live

CREATE INDEX IF NOT EXISTS idx_brain_aliases_norm
    ON brain_aliases (alias_norm);

CREATE INDEX IF NOT EXISTS idx_brain_subjects_lower_id
    ON brain_subjects (lower(subject_id));
