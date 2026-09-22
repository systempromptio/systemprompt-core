-- `managed_inventory_list (owner_id, entry_id)` has exactly the columns of
-- `managed_inventory_entries_pkey`, so the primary key already serves every
-- lookup and ordered scan it was created for. Migration 006 created it before
-- the table's key settled; the base schema no longer declares it.
--
-- It is not UNIQUE and backs no constraint.
DROP INDEX IF EXISTS managed_inventory_list;
