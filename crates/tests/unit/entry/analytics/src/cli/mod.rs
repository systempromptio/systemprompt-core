//! Tests for the analytics commands: dispatch over seeded data, table and
//! JSON output, CSV export, the human-readable formatters, the
//! `--since`/`--until` parsers and `core content popular`. Their
//! composition (declared commands, grants, help slot, descriptors) is
//! asserted by the facade tests in `unit/systemprompt/src/analytics_http.rs`.

mod analytics_commands_db;
mod analytics_seeded_conversations_db;
mod analytics_seeded_lists_db;
mod analytics_seeded_traffic_db;
mod analytics_seeded_usage_db;
mod analytics_show_seeded_db;
mod analytics_table_output_db;
mod content_popular_db;
mod cost_breakdown_outputs_db;
mod costs_summary_widening;
mod export_arms;
mod export_csv;
mod output_format;
mod overview;
mod time_parse;
mod time_parse_arms;
mod traffic_pages_navigation_db;
