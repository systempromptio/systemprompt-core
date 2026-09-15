CREATE TABLE IF NOT EXISTS analytics_feedback_snapshots (
 owner_id TEXT NOT NULL REFERENCES users(id),scope TEXT NOT NULL,window_days INTEGER NOT NULL,
 generation BIGINT NOT NULL,from_day DATE NOT NULL,to_day DATE NOT NULL,body JSONB NOT NULL,
 PRIMARY KEY(owner_id,scope,window_days)
);
