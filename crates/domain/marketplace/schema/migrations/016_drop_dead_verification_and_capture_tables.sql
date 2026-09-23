-- Git dependency verification, its source bindings and the baseline-capture
-- ledger were written only through the managed HTTP admin routes removed in
-- this release; nothing reads them. Their declarative schema is gone, so the
-- tables are dropped here. No other table, view or trigger depends on them.
--
-- The four tables this plane stopped declaring alongside these —
-- managed_api_operations and the consumer invocation evidence, attribution
-- projection and attribution history — are not dropped by core: a consumer
-- view may still read them, so the consumer drops them after redefining it.
DROP TABLE IF EXISTS managed_git_verifications;
DROP TABLE IF EXISTS managed_dependency_verifications;
DROP TABLE IF EXISTS managed_resource_git_bindings;
DROP TABLE IF EXISTS managed_inventory_captures;
