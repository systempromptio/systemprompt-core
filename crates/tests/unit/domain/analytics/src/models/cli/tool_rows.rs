//! Tests for tool rows.

use chrono::Utc;
use systemprompt_analytics::models::{
    ToolAgentUsageRow, ToolCaller, ToolErrorRow, ToolExecutionRow, ToolListRow, ToolStatsRow,
    ToolStatusBreakdownRow, ToolSummaryRow,
};
use systemprompt_identifiers::{AgentName, McpServerId, McpToolName};

mod tool_row_tests {
    use super::*;

    #[test]
    fn tool_list_row_stores_values() {
        let now = Utc::now();
        let row = ToolListRow {
            tool_name: McpToolName::new("web_search"),
            server_name: McpServerId::new("mcp-server-web"),
            execution_count: 5000,
            success_count: 4900,
            avg_time: 250.5,
            last_used: now,
        };

        assert_eq!(row.tool_name, "web_search");
        assert_eq!(row.server_name, "mcp-server-web");
        assert_eq!(row.execution_count, 5000);
        assert_eq!(row.success_count, 4900);
        assert!((row.avg_time - 250.5).abs() < f64::EPSILON);
    }

    #[test]
    fn tool_stats_row_stores_values() {
        let row = ToolStatsRow {
            total_tools: 50,
            total_executions: 100000,
            successful: 98000,
            failed: 1500,
            timeout: 500,
            avg_time: 150.0,
            p95_time: 500.0,
        };

        assert_eq!(row.total_tools, 50);
        assert_eq!(row.total_executions, 100000);
        assert_eq!(row.successful, 98000);
        assert_eq!(row.failed, 1500);
        assert_eq!(row.timeout, 500);
        assert!((row.avg_time - 150.0).abs() < f64::EPSILON);
        assert!((row.p95_time - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tool_summary_row_stores_values() {
        let row = ToolSummaryRow {
            total: 5000,
            successful: 4800,
            failed: 150,
            timeout: 50,
            avg_time: 200.0,
            p95_time: 600.0,
        };

        assert_eq!(row.total, 5000);
        assert_eq!(row.successful, 4800);
        assert_eq!(row.failed, 150);
        assert_eq!(row.timeout, 50);
        assert!((row.avg_time - 200.0).abs() < f64::EPSILON);
        assert!((row.p95_time - 600.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tool_status_breakdown_row_stores_values() {
        let row = ToolStatusBreakdownRow {
            status: "success".to_string(),
            status_count: 4800,
        };

        assert_eq!(row.status, "success");
        assert_eq!(row.status_count, 4800);
    }

    #[test]
    fn tool_error_row_stores_values() {
        let row = ToolErrorRow {
            error_msg: Some("Connection refused".to_string()),
            error_count: 25,
        };

        assert_eq!(row.error_msg, Some("Connection refused".to_string()));
        assert_eq!(row.error_count, 25);
    }

    #[test]
    fn tool_error_row_handles_none() {
        let row = ToolErrorRow {
            error_msg: None,
            error_count: 10,
        };

        assert!(row.error_msg.is_none());
    }

    #[test]
    fn tool_agent_usage_row_stores_values() {
        let row = ToolAgentUsageRow {
            caller: ToolCaller::Agent(AgentName::new("research-bot")),
            usage_count: 500,
        };

        assert_eq!(row.caller.to_string(), "research-bot");
        assert_eq!(row.usage_count, 500);
    }

    #[test]
    fn tool_caller_labels_non_agent_callers() {
        assert_eq!(ToolCaller::DirectCall.to_string(), "Direct Call");
        assert_eq!(ToolCaller::UnlinkedTask.to_string(), "Unlinked Task");
    }

    #[test]
    fn tool_execution_row_stores_values() {
        let now = Utc::now();
        let row = ToolExecutionRow {
            created_at: now,
            status: Some("success".to_string()),
            execution_time_ms: Some(150),
        };

        assert_eq!(row.created_at, now);
        assert_eq!(row.status, Some("success".to_string()));
        assert_eq!(row.execution_time_ms, Some(150));
    }

    #[test]
    fn tool_execution_row_handles_none() {
        let row = ToolExecutionRow {
            created_at: Utc::now(),
            status: None,
            execution_time_ms: None,
        };

        assert!(row.status.is_none());
        assert!(row.execution_time_ms.is_none());
    }
}
