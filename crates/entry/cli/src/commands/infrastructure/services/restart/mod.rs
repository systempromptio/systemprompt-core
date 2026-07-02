//! Service restart handlers shared by the single-target and batch restart
//! paths.
//!
//! Re-exports the per-target entry points ([`execute_api`], [`execute_agent`],
//! [`execute_mcp`]) and the batch entry points ([`execute_all_agents`],
//! [`execute_all_mcp`], [`execute_failed`]). Batch paths compute a
//! [`systemprompt_scheduler::RestartPlan`] from the observed service state
//! and drive the orchestrators composed in the parent module's `lifecycle`.

mod batch;
mod single;

use systemprompt_logging::CliService;

use super::get_api_port;

pub use batch::{execute_all_agents, execute_all_mcp, execute_failed};
pub use single::{execute_agent, execute_api, execute_mcp};

pub fn format_batch_message(
    service_label: &str,
    restarted: usize,
    failed: usize,
    quiet: bool,
) -> String {
    match (restarted, failed) {
        (0, 0) => {
            let msg = format!("No enabled {} found", service_label);
            if !quiet {
                CliService::info(&msg);
            }
            msg
        },
        (r, 0) => {
            let msg = format!("Restarted {} {}", r, service_label);
            if !quiet {
                CliService::success(&msg);
            }
            msg
        },
        (0, f) => {
            let msg = format!("Failed to restart {} {}", f, service_label);
            if !quiet {
                CliService::warning(&msg);
            }
            msg
        },
        (r, f) => {
            if !quiet {
                CliService::success(&format!("Restarted {} {}", r, service_label));
                CliService::warning(&format!("Failed to restart {} {}", f, service_label));
            }
            format!("Restarted {} {}, {} failed", r, service_label, f)
        },
    }
}
