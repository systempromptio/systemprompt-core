CREATE TABLE IF NOT EXISTS managed_installation_coverage (
 owner_id TEXT NOT NULL,resource_id TEXT NOT NULL,body JSONB NOT NULL,generation BIGINT NOT NULL,
 PRIMARY KEY(owner_id,resource_id),FOREIGN KEY(owner_id,resource_id) REFERENCES managed_resources(owner_id,id)
);
