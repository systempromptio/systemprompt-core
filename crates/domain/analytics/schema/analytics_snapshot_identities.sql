CREATE TABLE IF NOT EXISTS analytics_snapshot_identities (
 owner_id TEXT NOT NULL REFERENCES users(id),scope TEXT NOT NULL,day DATE NOT NULL,kind TEXT NOT NULL,identity TEXT NOT NULL,
 PRIMARY KEY(owner_id,scope,day,kind,identity)
);
