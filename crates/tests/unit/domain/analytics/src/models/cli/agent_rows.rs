//! Tests for agent rows.

use chrono::Utc;
use systemprompt_analytics::models::{
    AgentAiStatsRow, AgentErrorRow, AgentHourlyRow, AgentListRow, AgentStatsRow,
    AgentStatusBreakdownRow, AgentSummaryRow, AgentTaskRow, ConversationListRow, TimestampRow,
};
use systemprompt_identifiers::ContextId;

mod agent_row_tests {
    use super::*;

    #[test]
    fn agent_list_row_stores_values() {
        let now = Utc::now();
        let row = AgentListRow {
            agent_name: "research-assistant".to_string(),
            task_count: 1000,
            completed_count: 950,
            avg_execution_time_ms: 2500,
            total_cost_microdollars: 5000,
            last_active: now,
        };

        assert_eq!(row.agent_name, "research-assistant");
        assert_eq!(row.task_count, 1000);
        assert_eq!(row.completed_count, 950);
        assert_eq!(row.avg_execution_time_ms, 2500);
        assert_eq!(row.total_cost_microdollars, 5000);
    }

    #[test]
    fn agent_stats_row_stores_values() {
        let row = AgentStatsRow {
            total_agents: 25,
            total_tasks: 10000,
            completed_tasks: 9500,
            failed_tasks: 500,
            avg_execution_time_ms: 1500.5,
        };

        assert_eq!(row.total_agents, 25);
        assert_eq!(row.total_tasks, 10000);
        assert_eq!(row.completed_tasks, 9500);
        assert_eq!(row.failed_tasks, 500);
        assert!((row.avg_execution_time_ms - 1500.5).abs() < f64::EPSILON);
    }

    #[test]
    fn agent_ai_stats_row_stores_values() {
        let row = AgentAiStatsRow {
            total_ai_requests: 50000,
            total_cost_microdollars: 250000,
        };

        assert_eq!(row.total_ai_requests, 50000);
        assert_eq!(row.total_cost_microdollars, 250000);
    }

    #[test]
    fn agent_task_row_stores_values() {
        let now = Utc::now();
        let row = AgentTaskRow {
            started_at: now,
            status: Some("completed".to_string()),
            execution_time_ms: Some(1500),
        };

        assert_eq!(row.started_at, now);
        assert_eq!(row.status, Some("completed".to_string()));
        assert_eq!(row.execution_time_ms, Some(1500));
    }

    #[test]
    fn agent_status_breakdown_row_stores_values() {
        let row = AgentStatusBreakdownRow {
            status: "working".to_string(),
            status_count: 150,
        };

        assert_eq!(row.status, "working");
        assert_eq!(row.status_count, 150);
    }

    #[test]
    fn agent_error_row_stores_values() {
        let row = AgentErrorRow {
            error_type: Some("timeout".to_string()),
            error_count: 25,
        };

        assert_eq!(row.error_type, Some("timeout".to_string()));
        assert_eq!(row.error_count, 25);
    }

    #[test]
    fn agent_error_row_handles_none() {
        let row = AgentErrorRow {
            error_type: None,
            error_count: 5,
        };

        assert!(row.error_type.is_none());
    }

    #[test]
    fn agent_hourly_row_stores_values() {
        let row = AgentHourlyRow {
            task_hour: 14,
            task_count: 250,
        };

        assert_eq!(row.task_hour, 14);
        assert_eq!(row.task_count, 250);
    }

    #[test]
    fn agent_summary_row_stores_values() {
        let row = AgentSummaryRow {
            total_tasks: 1000,
            completed: 900,
            failed: 100,
            avg_time: 1500.0,
        };

        assert_eq!(row.total_tasks, 1000);
        assert_eq!(row.completed, 900);
        assert_eq!(row.failed, 100);
        assert!((row.avg_time - 1500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn conversation_list_row_stores_values() {
        let now = Utc::now();
        let row = ConversationListRow {
            context_id: ContextId::new("ctx_conv".to_string()),
            name: Some("Support Chat".to_string()),
            task_count: 5,
            message_count: 25,
            created_at: now,
            updated_at: now,
        };

        assert_eq!(row.context_id.as_str(), "ctx_conv");
        assert_eq!(row.name, Some("Support Chat".to_string()));
        assert_eq!(row.task_count, 5);
        assert_eq!(row.message_count, 25);
    }

    #[test]
    fn timestamp_row_stores_values() {
        let now = Utc::now();
        let row = TimestampRow { timestamp: now };

        assert_eq!(row.timestamp, now);
    }
}
