//! Table tests for the pure lifecycle plans: `StartupPlan::compute` and
//! `RestartPlan::compute`.

use systemprompt_scheduler::{
    RestartPlan, RestartScope, ServiceSnapshot, ServiceType, StartupPlan, StartupRequest,
};

fn snapshot(
    service_type: ServiceType,
    id: &str,
    name: &str,
    enabled: bool,
    healthy: bool,
) -> ServiceSnapshot {
    ServiceSnapshot {
        service_type,
        id: id.to_owned(),
        name: name.to_owned(),
        enabled,
        healthy,
    }
}

mod startup_plan {
    use super::*;

    #[test]
    fn all_targets_runs_migrations_and_starts_api() {
        let plan = StartupPlan::compute(StartupRequest {
            api: true,
            agents: true,
            mcp: true,
            skip_migrate: false,
        });

        assert!(plan.run_migrations);
        assert!(plan.start_api);
        assert!(!plan.agents_standalone_notice);
        assert!(!plan.mcp_standalone_notice);
    }

    #[test]
    fn skip_migrate_disables_migrations() {
        let plan = StartupPlan::compute(StartupRequest {
            api: true,
            agents: false,
            mcp: false,
            skip_migrate: true,
        });

        assert!(!plan.run_migrations);
        assert!(plan.start_api);
    }

    #[test]
    fn agents_without_api_yields_notice_only() {
        let plan = StartupPlan::compute(StartupRequest {
            api: false,
            agents: true,
            mcp: false,
            skip_migrate: false,
        });

        assert!(!plan.start_api);
        assert!(plan.agents_standalone_notice);
        assert!(!plan.mcp_standalone_notice);
    }

    #[test]
    fn mcp_without_api_yields_notice_only() {
        let plan = StartupPlan::compute(StartupRequest {
            api: false,
            agents: false,
            mcp: true,
            skip_migrate: false,
        });

        assert!(!plan.start_api);
        assert!(!plan.agents_standalone_notice);
        assert!(plan.mcp_standalone_notice);
    }

    #[test]
    fn agents_and_mcp_with_api_suppresses_notices() {
        let plan = StartupPlan::compute(StartupRequest {
            api: true,
            agents: true,
            mcp: true,
            skip_migrate: true,
        });

        assert!(plan.start_api);
        assert!(!plan.agents_standalone_notice);
        assert!(!plan.mcp_standalone_notice);
    }
}

mod restart_plan {
    use super::*;

    fn mixed_snapshot() -> Vec<ServiceSnapshot> {
        vec![
            snapshot(ServiceType::Agent, "agent-1", "Agent One", true, true),
            snapshot(ServiceType::Agent, "agent-2", "Agent Two", true, false),
            snapshot(ServiceType::Agent, "agent-3", "Agent Three", false, false),
            snapshot(ServiceType::Mcp, "mcp-1", "mcp-1", true, true),
            snapshot(ServiceType::Mcp, "mcp-2", "mcp-2", true, false),
            snapshot(ServiceType::Mcp, "mcp-3", "mcp-3", false, true),
        ]
    }

    #[test]
    fn all_agents_picks_only_enabled_agents() {
        let plan = RestartPlan::compute(RestartScope::AllAgents, &mixed_snapshot());

        let ids: Vec<&str> = plan.targets.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["agent-1", "agent-2"]);
        assert!(
            plan.targets
                .iter()
                .all(|t| t.service_type == ServiceType::Agent)
        );
    }

    #[test]
    fn all_mcp_picks_only_enabled_mcp_servers() {
        let plan = RestartPlan::compute(RestartScope::AllMcp, &mixed_snapshot());

        let ids: Vec<&str> = plan.targets.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["mcp-1", "mcp-2"]);
    }

    #[test]
    fn failed_picks_unhealthy_enabled_services_of_both_kinds() {
        let plan = RestartPlan::compute(RestartScope::Failed, &mixed_snapshot());

        let ids: Vec<&str> = plan.targets.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["agent-2", "mcp-2"]);
    }

    #[test]
    fn disabled_services_are_excluded_even_when_unhealthy() {
        let plan = RestartPlan::compute(RestartScope::Failed, &mixed_snapshot());

        assert!(plan.targets.iter().all(|t| t.id != "agent-3"));
    }

    #[test]
    fn target_keeps_id_and_display_name_distinct() {
        let plan = RestartPlan::compute(
            RestartScope::AllAgents,
            &[snapshot(ServiceType::Agent, "agent-1", "Agent One", true, true)],
        );

        assert_eq!(plan.targets.len(), 1);
        assert_eq!(plan.targets[0].id, "agent-1");
        assert_eq!(plan.targets[0].name, "Agent One");
    }

    #[test]
    fn empty_snapshot_yields_empty_plan() {
        let plan = RestartPlan::compute(RestartScope::Failed, &[]);
        assert!(plan.targets.is_empty());
    }

    #[test]
    fn snapshot_order_is_preserved() {
        let plan = RestartPlan::compute(
            RestartScope::Failed,
            &[
                snapshot(ServiceType::Mcp, "mcp-z", "mcp-z", true, false),
                snapshot(ServiceType::Agent, "agent-a", "Agent A", true, false),
            ],
        );

        let ids: Vec<&str> = plan.targets.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["mcp-z", "agent-a"]);
    }
}
