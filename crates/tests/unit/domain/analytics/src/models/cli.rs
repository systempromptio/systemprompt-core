//! Tests for CLI analytics model types.

use chrono::Utc;
use systemprompt_analytics::models::{
    ActiveSessionCountRow, AgentAiStatsRow, AgentErrorRow, AgentHourlyRow, AgentListRow,
    AgentStatsRow, AgentStatusBreakdownRow, AgentSummaryRow, AgentTaskRow, ConversationListRow,
    ConversationStatsRow, LiveSessionRow, MessageCountRow, OverviewActiveSessionRow,
    OverviewAgentRow, OverviewConversationRow, OverviewCostRow, OverviewRequestRow, OverviewToolRow,
    OverviewTotalSessionRow, SessionStatsRow, SessionTrendRow, TaskStatsRow, TimestampRow,
    ToolAgentUsageRow, ToolErrorRow, ToolExistsRow, ToolExecutionRow, ToolListRow,
    ToolStatusBreakdownRow, ToolStatsRow, ToolSummaryRow,
};
use systemprompt_identifiers::{ContextId, SessionId, UserId};

mod overview_row_tests {
    use super::*;

    #[test]
    fn conversation_row_stores_count() {
        let row = OverviewConversationRow { count: 150 };
        assert_eq!(row.count, 150);
    }

    #[test]
    fn conversation_row_is_copy() {
        let row = OverviewConversationRow { count: 100 };
        let copied = row;
        assert_eq!(row.count, copied.count);
    }

    #[test]
    fn conversation_row_is_debug() {
        let row = OverviewConversationRow { count: 50 };
        let debug_str = format!("{:?}", row);
        assert!(debug_str.contains("OverviewConversationRow"));
    }

    #[test]
    fn conversation_row_serializes() {
        let row = OverviewConversationRow { count: 75 };
        let json = serde_json::to_string(&row).unwrap();
        assert!(json.contains("75"));
    }

    #[test]
    fn agent_row_stores_values() {
        let row = OverviewAgentRow {
            active_agents: 10,
            total_tasks: 500,
            completed_tasks: 450,
        };

        assert_eq!(row.active_agents, 10);
        assert_eq!(row.total_tasks, 500);
        assert_eq!(row.completed_tasks, 450);
    }

    #[test]
    fn agent_row_is_copy() {
        let row = OverviewAgentRow {
            active_agents: 5,
            total_tasks: 100,
            completed_tasks: 90,
        };
        let copied = row;
        assert_eq!(row.active_agents, copied.active_agents);
    }

    #[test]
    fn request_row_stores_values() {
        let row = OverviewRequestRow {
            total: 10000,
            total_tokens: Some(500000),
            avg_latency: Some(150.5),
        };

        assert_eq!(row.total, 10000);
        assert_eq!(row.total_tokens, Some(500000));
        assert!((row.avg_latency.unwrap() - 150.5).abs() < f64::EPSILON);
    }

    #[test]
    fn request_row_handles_none_values() {
        let row = OverviewRequestRow {
            total: 100,
            total_tokens: None,
            avg_latency: None,
        };

        assert_eq!(row.total, 100);
        assert!(row.total_tokens.is_none());
        assert!(row.avg_latency.is_none());
    }

    #[test]
    fn tool_row_stores_values() {
        let row = OverviewToolRow {
            total: 5000,
            successful: 4800,
        };

        assert_eq!(row.total, 5000);
        assert_eq!(row.successful, 4800);
    }

    #[test]
    fn active_session_row_stores_count() {
        let row = OverviewActiveSessionRow { count: 250 };
        assert_eq!(row.count, 250);
    }

    #[test]
    fn total_session_row_stores_count() {
        let row = OverviewTotalSessionRow { count: 50000 };
        assert_eq!(row.count, 50000);
    }

    #[test]
    fn cost_row_stores_value() {
        let row = OverviewCostRow { cost: Some(12500) };
        assert_eq!(row.cost, Some(12500));
    }

    #[test]
    fn cost_row_handles_none() {
        let row = OverviewCostRow { cost: None };
        assert!(row.cost.is_none());
    }
}

mod session_row_tests {
    use super::*;

    #[test]
    fn session_stats_row_stores_values() {
        let row = SessionStatsRow {
            total_sessions: 10000,
            unique_users: 5000,
            avg_duration: Some(300.5),
            avg_requests: Some(15.2),
            conversions: 500,
        };

        assert_eq!(row.total_sessions, 10000);
        assert_eq!(row.unique_users, 5000);
        assert!((row.avg_duration.unwrap() - 300.5).abs() < f64::EPSILON);
        assert!((row.avg_requests.unwrap() - 15.2).abs() < f64::EPSILON);
        assert_eq!(row.conversions, 500);
    }

    #[test]
    fn session_stats_row_is_copy() {
        let row = SessionStatsRow {
            total_sessions: 100,
            unique_users: 50,
            avg_duration: Some(60.0),
            avg_requests: Some(5.0),
            conversions: 10,
        };
        let copied = row;
        assert_eq!(row.total_sessions, copied.total_sessions);
    }

    #[test]
    fn session_stats_row_handles_none() {
        let row = SessionStatsRow {
            total_sessions: 50,
            unique_users: 25,
            avg_duration: None,
            avg_requests: None,
            conversions: 5,
        };

        assert!(row.avg_duration.is_none());
        assert!(row.avg_requests.is_none());
    }

    #[test]
    fn active_session_count_row_stores_count() {
        let row = ActiveSessionCountRow { count: 125 };
        assert_eq!(row.count, 125);
    }

    #[test]
    fn live_session_row_stores_values() {
        let now = Utc::now();
        let row = LiveSessionRow {
            session_id: SessionId::new("sess_live".to_string()),
            user_type: Some("authenticated".to_string()),
            started_at: now,
            duration_seconds: Some(300),
            request_count: Some(25),
            last_activity_at: now,
        };

        assert_eq!(row.session_id.as_str(), "sess_live");
        assert_eq!(row.user_type, Some("authenticated".to_string()));
        assert_eq!(row.duration_seconds, Some(300));
        assert_eq!(row.request_count, Some(25));
    }

    #[test]
    fn live_session_row_is_clone() {
        let now = Utc::now();
        let row = LiveSessionRow {
            session_id: SessionId::new("sess_clone".to_string()),
            user_type: None,
            started_at: now,
            duration_seconds: None,
            request_count: None,
            last_activity_at: now,
        };
        let cloned = row.clone();
        assert_eq!(row.session_id.as_str(), cloned.session_id.as_str());
    }

    #[test]
    fn session_trend_row_stores_values() {
        let now = Utc::now();
        let row = SessionTrendRow {
            started_at: now,
            user_id: Some(UserId::new("user_trend".to_string())),
            duration_seconds: Some(180),
        };

        assert_eq!(row.started_at, now);
        assert!(row.user_id.is_some());
        assert_eq!(row.duration_seconds, Some(180));
    }

    #[test]
    fn session_trend_row_is_clone() {
        let now = Utc::now();
        let row = SessionTrendRow {
            started_at: now,
            user_id: None,
            duration_seconds: None,
        };
        let cloned = row.clone();
        assert_eq!(row.started_at, cloned.started_at);
    }
}

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
    fn agent_list_row_is_clone() {
        let row = AgentListRow {
            agent_name: "test".to_string(),
            task_count: 10,
            completed_count: 9,
            avg_execution_time_ms: 100,
            total_cost_microdollars: 50,
            last_active: Utc::now(),
        };
        let cloned = row.clone();
        assert_eq!(row.agent_name, cloned.agent_name);
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
    fn agent_stats_row_is_copy() {
        let row = AgentStatsRow {
            total_agents: 5,
            total_tasks: 100,
            completed_tasks: 90,
            failed_tasks: 10,
            avg_execution_time_ms: 500.0,
        };
        let copied = row;
        assert_eq!(row.total_agents, copied.total_agents);
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
    fn conversation_stats_row_stores_values() {
        let row = ConversationStatsRow {
            total_contexts: 5000,
        };

        assert_eq!(row.total_contexts, 5000);
    }

    #[test]
    fn task_stats_row_stores_values() {
        let row = TaskStatsRow {
            total_tasks: 25000,
            avg_execution_time_ms: Some(1200.5),
        };

        assert_eq!(row.total_tasks, 25000);
        assert!((row.avg_execution_time_ms.unwrap() - 1200.5).abs() < f64::EPSILON);
    }

    #[test]
    fn task_stats_row_handles_none() {
        let row = TaskStatsRow {
            total_tasks: 0,
            avg_execution_time_ms: None,
        };

        assert!(row.avg_execution_time_ms.is_none());
    }

    #[test]
    fn message_count_row_stores_values() {
        let row = MessageCountRow {
            total_messages: 100000,
        };

        assert_eq!(row.total_messages, 100000);
    }

    #[test]
    fn timestamp_row_stores_values() {
        let now = Utc::now();
        let row = TimestampRow { timestamp: now };

        assert_eq!(row.timestamp, now);
    }
}

mod tool_row_tests {
    use super::*;

    #[test]
    fn tool_list_row_stores_values() {
        let now = Utc::now();
        let row = ToolListRow {
            tool_name: "web_search".to_string(),
            server_name: "mcp-server-web".to_string(),
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
    fn tool_list_row_is_clone() {
        let row = ToolListRow {
            tool_name: "test".to_string(),
            server_name: "server".to_string(),
            execution_count: 10,
            success_count: 9,
            avg_time: 100.0,
            last_used: Utc::now(),
        };
        let cloned = row.clone();
        assert_eq!(row.tool_name, cloned.tool_name);
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
    fn tool_stats_row_is_copy() {
        let row = ToolStatsRow {
            total_tools: 10,
            total_executions: 1000,
            successful: 980,
            failed: 15,
            timeout: 5,
            avg_time: 100.0,
            p95_time: 300.0,
        };
        let copied = row;
        assert_eq!(row.total_tools, copied.total_tools);
    }

    #[test]
    fn tool_exists_row_stores_count() {
        let row = ToolExistsRow { count: 1 };
        assert_eq!(row.count, 1);
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
            agent_name: Some("research-bot".to_string()),
            usage_count: 500,
        };

        assert_eq!(row.agent_name, Some("research-bot".to_string()));
        assert_eq!(row.usage_count, 500);
    }

    #[test]
    fn tool_agent_usage_row_handles_none() {
        let row = ToolAgentUsageRow {
            agent_name: None,
            usage_count: 100,
        };

        assert!(row.agent_name.is_none());
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
