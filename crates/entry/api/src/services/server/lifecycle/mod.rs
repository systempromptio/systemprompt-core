//! Server startup reconciliation and lifecycle wiring.
//!
//! Reconciles enabled agents and system MCP services into a running state,
//! bridges runtime startup events, and initializes the scheduler before the
//! server begins accepting traffic.

mod agents;
mod event_bridge;
mod reconciliation;
mod scheduler;

pub(super) use agents::reconcile_agents;
pub(super) use event_bridge::start_event_bridge;
pub(super) use reconciliation::reconcile_system_services;
pub(super) use scheduler::initialize_scheduler;
