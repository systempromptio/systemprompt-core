//! Integration tests for `systemprompt-api` HTTP route handlers.

#[cfg(test)]
mod agent_http_member;

#[cfg(test)]
mod routes_marketplace_mounted;

#[cfg(test)]
mod routes_gateway_auth;

#[cfg(test)]
mod server_mount_gateway_and_mcp_scope;

#[cfg(test)]
mod server_mount_analytics;

#[cfg(test)]
mod server_static_site_auth;

#[cfg(test)]
mod site_fallback_ip_ban;

#[cfg(test)]
mod server_mount_notifications;

#[cfg(test)]
mod routes_agent_mounted;

#[cfg(test)]
mod routes_content_mounted;

#[cfg(test)]
mod routes_brain_mounted;

#[cfg(test)]
mod routes_role_gateway;

#[cfg(test)]
mod gateway_admitted_routes;

#[cfg(test)]
mod server_declared_groups;

#[cfg(test)]
mod gateway_overhead_metrics;

#[cfg(test)]
mod load_shed_outermost;

#[cfg(test)]
mod load_shed_probe_cap;

#[cfg(test)]
mod server_load_shed;

#[cfg(test)]
mod routes_mcp_registry_mounted;

#[cfg(test)]
mod routes_wellknown;

#[cfg(test)]
mod routes_wellknown_agent_cards_populated;

#[cfg(test)]
mod routes_agent_registry;

#[cfg(test)]
mod routes_agent_registry_populated;

#[cfg(test)]
mod routes_agent_read_outage;
#[cfg(test)]
mod routes_agent_tasks;

#[cfg(test)]
mod routes_admin_keys;

#[cfg(test)]
mod routes_users_sessions;

#[cfg(test)]
mod routes_misc;

#[cfg(test)]
mod routes_gateway;

#[cfg(test)]
mod routes_gateway_authed;

#[cfg(test)]
mod routes_gateway_manifest_skills;

#[cfg(test)]
mod routes_oauth_discovery;

#[cfg(test)]
mod routes_stream;

#[cfg(test)]
mod routes_sync;

#[cfg(test)]
mod routes_proxy;

#[cfg(test)]
mod routes_health_and_oauth_wellknown;

#[cfg(test)]
mod routes_oauth_public;

#[cfg(test)]
mod routes_admin_cli;

#[cfg(test)]
mod routes_agent_contexts;

#[cfg(test)]
mod server_boot;

#[cfg(test)]
mod boot_missing_providers;

#[cfg(test)]
mod server_early_bind;

#[cfg(test)]
mod server_run_loop;

#[cfg(test)]
mod services_quota_policy;

#[cfg(test)]
mod services_quota_sharding;

#[cfg(test)]
mod routes_bridge_data;

#[cfg(test)]
mod gateway_http_member;
#[cfg(test)]
mod identity_http_member;
#[cfg(test)]
mod marketplace_consumer_mount;
#[cfg(test)]
mod routes_gateway_bridge_jwt;

#[cfg(test)]
mod routes_oauth_pkce_flow;

#[cfg(test)]
mod middleware_edges;

#[cfg(test)]
mod gateway_extract_units;

#[cfg(test)]
mod stream_tap_accumulator;

#[cfg(test)]
mod protocol_matrix;

#[cfg(test)]
mod routes_agent_runs;

#[cfg(test)]
mod routes_agent_runs_mounted;

#[cfg(test)]
mod documents;
#[cfg(test)]
mod documents_access_requests;
#[cfg(test)]
mod documents_collab;
#[cfg(test)]
mod documents_comment_cells;
#[cfg(test)]
mod documents_comment_edit;
#[cfg(test)]
mod documents_drive_policy;
#[cfg(test)]
mod documents_invites;
#[cfg(test)]
mod documents_parity;
#[cfg(test)]
mod documents_pdf;
#[cfg(test)]
mod documents_people;
#[cfg(test)]
mod documents_public_links;
#[cfg(test)]
mod documents_read_parity;
#[cfg(test)]
mod documents_rendered;
#[cfg(test)]
mod documents_review;
#[cfg(test)]
mod documents_sharing;
#[cfg(test)]
mod documents_subject_levels;
#[cfg(test)]
mod documents_ui_session;
#[cfg(test)]
mod files_http_surface;
#[cfg(test)]
mod internal_links;
#[cfg(test)]
mod internal_links_worker;
#[cfg(test)]
mod member_surfaces;
#[cfg(test)]
mod routes_oauth_token;

#[cfg(test)]
mod server_gateway_disabled;

#[cfg(test)]
mod common;

#[cfg(test)]
mod oauth_register_owner_fk_present;

#[cfg(test)]
mod session_middleware_persists_anon;

#[cfg(test)]
mod routes_mcp_unauth_challenge;

#[cfg(test)]
mod routes_oauth_webauthn_complete;

#[cfg(test)]
mod routes_oauth_token_exchange;

#[cfg(test)]
mod routes_health_discovery;

#[cfg(test)]
mod routes_agent_webhook;

#[cfg(test)]
mod routes_agent_body_limit;
#[cfg(test)]
mod routes_proxy_forward;

#[cfg(test)]
mod proxy_internal_session;

#[cfg(test)]
mod routes_sync_files;

#[cfg(test)]
mod routes_agent_artifacts;

#[cfg(test)]
mod messaging_dispatch;
#[cfg(test)]
mod messaging_linking;

#[cfg(test)]
mod routes_oauth_authorize;

#[cfg(test)]
mod routes_oauth_authorize_session;

#[cfg(test)]
mod routes_oauth_logout;

#[cfg(test)]
mod middleware_session_lifecycle;

#[cfg(test)]
mod middleware_analytics_detection;

#[cfg(test)]
mod middleware_behavioral_detection;

#[cfg(test)]
mod middleware_jwt_context;

#[cfg(test)]
mod request_authenticator_paths;

#[cfg(test)]
mod health_monitor;

#[cfg(test)]
mod static_files_serving;

#[cfg(test)]
mod static_files_layers;

#[cfg(test)]
mod gateway_journal_drain;

#[cfg(test)]
mod gateway_pipeline;

#[cfg(test)]
mod gateway_recall;

#[cfg(test)]
mod gateway_recall_cases;

#[cfg(test)]
mod gateway_failover;

#[cfg(test)]
mod gateway_tool_result_artifacts;

#[cfg(test)]
mod gateway_safety_scanner_failure;

#[cfg(test)]
mod gateway_safety_scanner_config;

#[cfg(test)]
mod gateway_safety_redaction;

#[cfg(test)]
mod gateway_scan_sent_bytes;

#[cfg(test)]
mod gateway_matrix;

#[cfg(test)]
mod gateway_matrix_anthropic_in;

#[cfg(test)]
mod gateway_matrix_degenerate;

#[cfg(test)]
mod gateway_matrix_openai_chat_in;

#[cfg(test)]
mod gateway_matrix_openai_responses_in;

#[cfg(test)]
mod gateway_matrix_inbound;

#[cfg(test)]
mod gateway_matrix_anthropic_terminal;

#[cfg(test)]
mod gateway_matrix_openai_responses_terminal;

#[cfg(test)]
mod gateway_matrix_null_fields;

#[cfg(test)]
mod gateway_upstream_error_map;

#[cfg(test)]
mod gateway_empty_upstream_body;

#[cfg(test)]
mod gateway_stream_abort;

#[cfg(test)]
mod gateway_stream_usage;

#[cfg(test)]
mod routes_bridge_plugin_archive;
#[cfg(test)]
mod routes_bridge_plugin_file;

#[cfg(test)]
mod routes_bridge_manifest_catalog;

#[cfg(test)]
mod routes_bridge_manifest_config_reload;

#[cfg(test)]
mod routes_proxy_mcp_executions;

#[cfg(test)]
mod routes_agent_cards;

#[cfg(test)]
mod proxy_auth_access;

#[cfg(test)]
mod proxy_mcp_headers;

#[cfg(test)]
mod proxy_audit_tap;

#[cfg(test)]
mod routes_oauth_token_grants_happy;

#[cfg(test)]
mod routes_oauth_grant_resource;

#[cfg(test)]
mod routes_oauth_clients;

#[cfg(test)]
mod routes_oauth_introspect_revoke;

#[cfg(test)]
mod routes_oauth_device_link;
#[cfg(test)]
mod routes_oauth_person_ui;
#[cfg(test)]
mod routes_oauth_revoke_refresh;

#[cfg(test)]
mod routes_oauth_ui_revoke;

#[cfg(test)]
mod routes_oauth_userinfo;

#[cfg(test)]
mod routes_oauth_webauthn;

#[cfg(test)]
mod routes_oauth_token_exchange_oidc;

#[cfg(test)]
mod routes_agent_notifications;

#[cfg(test)]
mod routes_agent_context_events;

#[cfg(test)]
mod routes_agent_webhook_broadcast;

#[cfg(test)]
mod routes_agent_webhook_ownership;

#[cfg(test)]
mod routes_agent_contexts_crud;

#[cfg(test)]
mod gateway_messages_more;

#[cfg(test)]
mod gateway_otel_ingest;

#[cfg(test)]
mod gateway_bridge_models_more;

#[cfg(test)]
mod gateway_dispatch_more;

#[cfg(test)]
mod server_extension_readiness;
#[cfg(test)]
mod server_readiness_db_loss;
#[cfg(test)]
mod server_readiness_metrics;

#[cfg(test)]
mod server_health_more;

#[cfg(test)]
mod static_content_more;

#[cfg(test)]
mod middleware_more;

#[cfg(test)]
mod gateway_scanner_without_ai;
#[cfg(test)]
mod gateway_without_agents;
#[cfg(test)]
mod proxy_cli_attribution;
#[cfg(test)]
mod proxy_external_mcp;
#[cfg(test)]
mod proxy_mcp_without_contexts;

#[cfg(test)]
mod proxy_more;

#[cfg(test)]
mod api_error_units;

#[cfg(test)]
mod routes_admin_cli_subprocess;

#[cfg(test)]
mod routes_admin_keys_seeded;

#[cfg(test)]
mod routes_mcp_discovery_populated;

#[cfg(test)]
mod routes_content_link_analytics_admin;

#[cfg(test)]
mod routes_oauth_webauthn_link;

#[cfg(test)]
mod routes_stream_delivery;

#[cfg(test)]
mod gateway_bridge_release_routes;

#[cfg(test)]
mod gateway_enabled_routes;

#[cfg(test)]
mod gateway_profile_usage_seeded;

#[cfg(test)]
mod jwt_user_state_denial;

#[cfg(test)]
mod gateway_limits_by_catalog_id;

#[cfg(test)]
mod gateway_limits_passthrough;

#[cfg(test)]
mod gateway_dispatch_success;

#[cfg(test)]
mod gateway_caller_credential;

#[cfg(test)]
mod gateway_malformed_tool_choice;

#[cfg(test)]
mod gateway_google_credentials;

#[cfg(test)]
mod api_error_redaction;

#[cfg(test)]
mod gateway_attribution;

#[cfg(test)]
mod gateway_traffic;

#[cfg(test)]
mod gateway_quota_reservation;

#[cfg(test)]
mod gateway_scope_routing;

#[cfg(test)]
mod gateway_api_key_limits;

#[cfg(test)]
mod gateway_rate_windows;

#[cfg(test)]
mod gateway_principal_parity;

#[cfg(test)]
mod gateway_api_key_session_binding;

#[cfg(test)]
mod process_leases;

#[cfg(test)]
mod documents_agent_cap_live;

#[cfg(test)]
mod routes_uploads;

#[cfg(test)]
mod routes_uploads_tokens;

#[cfg(test)]
mod gateway_provider_credentials;

#[cfg(test)]
mod routes_issues;

#[cfg(test)]
mod gateway_bridge_notifications;

#[cfg(test)]
mod proxy_owner_forward;

#[cfg(test)]
mod documents_import_export;

#[cfg(test)]
mod ownership_sweep;

#[cfg(test)]
mod content_link_analytics_admin;
