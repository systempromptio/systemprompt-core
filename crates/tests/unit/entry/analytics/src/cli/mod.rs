//! Tests for the analytics commands: dispatch over seeded data, table and
//! JSON output, CSV export, the human-readable formatters, the
//! `--since`/`--until` parsers, `core content popular`, and the kernel
//! registration of both.

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
mod registration;
mod time_parse;
mod time_parse_arms;
mod traffic_pages_navigation_db;
