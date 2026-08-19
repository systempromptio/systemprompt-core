//! Tests for `infra` command builders.

mod db_app_ctx;
mod db_commands_db;
mod db_migrate_fresh_db;
mod db_migrate_repair_db;
mod db_migration_drift_db;
mod jobs_cleanup_history_db;
mod jobs_logs_app_ctx;
mod jobs_toggle_db;
mod logs;
mod logs_commands_db;
mod logs_request_and_previews_db;
mod logs_seeded_trace_db;
mod logs_show_text_db;
mod logs_stream_cleanup_db;
mod logs_trace_ai_mcp_db;
mod logs_trace_render_db;
mod services_app_ctx;
mod services_commands;
mod services_restart_ctx;
mod services_start_ctx;
mod services_status_output;
