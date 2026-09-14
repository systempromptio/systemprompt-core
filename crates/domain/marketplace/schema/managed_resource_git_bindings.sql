CREATE TABLE IF NOT EXISTS managed_resource_git_bindings (
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    relative_root TEXT NOT NULL CHECK (length(relative_root) BETWEEN 1 AND 4096),
    bound_by TEXT NOT NULL REFERENCES users(id),
    bound_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id, resource_id),
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id),
    FOREIGN KEY(owner_id, source_id) REFERENCES managed_sources(owner_id, id)
);
DROP TRIGGER IF EXISTS managed_resource_git_bindings_immutable ON managed_resource_git_bindings;
CREATE TRIGGER managed_resource_git_bindings_immutable BEFORE UPDATE ON managed_resource_git_bindings FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
