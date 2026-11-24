//! Tests for `systemprompt-agent-entry`: the agent conversation route
//! builders and request types, and the `admin agents`, `core contexts` and
//! `core artifacts` commands driven through member hosts holding each group's
//! grants over a kernel that holds only what the agent members provide.

#[cfg(test)]
mod admin_agent_logs_all_filtering_db;
#[cfg(test)]
mod admin_agents_create_full_fs;
#[cfg(test)]
mod admin_agents_edit_flags_fs;
#[cfg(test)]
mod admin_agents_message_stream;
#[cfg(test)]
mod admin_agents_profile_fs;
#[cfg(test)]
mod agent_logs_follow_lifecycle;
#[cfg(test)]
mod agents_create_prompts;
#[cfg(test)]
mod agents_delete;
#[cfg(test)]
mod agents_delete_flow;
#[cfg(test)]
mod agents_logs_db;
#[cfg(test)]
mod agents_logs_disk_prompts;
#[cfg(test)]
mod agents_logs_disk_selection;
#[cfg(test)]
mod agents_registry_projection;
#[cfg(test)]
mod agents_shared_args;
#[cfg(test)]
mod agents_shared_prompt;
#[cfg(test)]
mod agents_table_output_fs;
#[cfg(test)]
mod agents_validate_checks;
#[cfg(test)]
mod common;
#[cfg(test)]
mod core_artifacts_db;
#[cfg(test)]
mod core_contexts_db;
#[cfg(test)]
mod core_contexts_public_lifecycle;
#[cfg(test)]
mod http_error_mapping;
#[cfg(test)]
mod messaging_helpers;
#[cfg(test)]
mod routes;
#[cfg(test)]
mod routes_agent_responses;
