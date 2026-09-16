CREATE TABLE IF NOT EXISTS managed_inventory_authoring_heads (
    owner_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    PRIMARY KEY(owner_id,entry_id),
    FOREIGN KEY(owner_id,entry_id) REFERENCES managed_inventory_entries(owner_id,entry_id),
    FOREIGN KEY(owner_id,revision_id) REFERENCES managed_revisions(owner_id,id)
);
DROP TRIGGER IF EXISTS managed_inventory_bindings_immutable ON managed_inventory_bindings;
CREATE TRIGGER managed_inventory_bindings_immutable BEFORE UPDATE ON managed_inventory_bindings FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
