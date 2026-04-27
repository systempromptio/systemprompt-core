//! Tests for top entity, conversation, content, error, and activity trend model
//! types.

use chrono::Utc;
use systemprompt_analytics::{
    ActivityTrend, ContentStat, ConversationByAgent, ConversationSummary, ConversationTrend,
    ErrorSummary, RecentConversation, TopAgent, TopTool, TopUser,
};
use systemprompt_identifiers::{ContextId, UserId};

mod top_entity_tests {
    use super::*;

    #[test]
    fn top_user_stores_values() {
        let user = TopUser {
            user_id: UserId::new("user_top".to_string()),
            user_name: "John Doe".to_string(),
            session_count: 100,
            task_count: 500,
            ai_request_count: 1000,
            total_cost: 99.99,
        };

        assert_eq!(user.user_id.as_str(), "user_top");
        assert_eq!(user.user_name, "John Doe");
        assert_eq!(user.session_count, 100);
        assert_eq!(user.task_count, 500);
        assert_eq!(user.ai_request_count, 1000);
        assert!((user.total_cost - 99.99).abs() < f64::EPSILON);
    }

    #[test]
    fn top_agent_stores_values() {
        let agent = TopAgent {
            agent_name: "research-assistant".to_string(),
            task_count: 1000,
            success_rate: 0.95,
            avg_duration_ms: 5000,
        };

        assert_eq!(agent.agent_name, "research-assistant");
        assert_eq!(agent.task_count, 1000);
        assert!((agent.success_rate - 0.95).abs() < f64::EPSILON);
        assert_eq!(agent.avg_duration_ms, 5000);
    }

    #[test]
    fn top_tool_stores_values() {
        let tool = TopTool {
            tool_name: "web_search".to_string(),
            execution_count: 5000,
            success_rate: 0.98,
            avg_duration_ms: 1500,
        };

        assert_eq!(tool.tool_name, "web_search");
        assert_eq!(tool.execution_count, 5000);
        assert!((tool.success_rate - 0.98).abs() < f64::EPSILON);
    }
}

mod conversation_stats_tests {
    use super::*;

    #[test]
    fn conversation_summary_stores_values() {
        let summary = ConversationSummary {
            total_conversations: 10000,
            active_conversations: 500,
            completed_conversations: 9500,
            avg_messages_per_conversation: 15.5,
            avg_duration_minutes: 10.2,
        };

        assert_eq!(summary.total_conversations, 10000);
        assert_eq!(summary.active_conversations, 500);
        assert_eq!(summary.completed_conversations, 9500);
        assert!((summary.avg_messages_per_conversation - 15.5).abs() < f64::EPSILON);
        assert!((summary.avg_duration_minutes - 10.2).abs() < f64::EPSILON);
    }

    #[test]
    fn conversation_trend_stores_values() {
        let now = Utc::now();
        let trend = ConversationTrend {
            date: now,
            new_conversations: 50,
            completed_conversations: 45,
            total_messages: 750,
        };

        assert_eq!(trend.date, now);
        assert_eq!(trend.new_conversations, 50);
        assert_eq!(trend.completed_conversations, 45);
        assert_eq!(trend.total_messages, 750);
    }

    #[test]
    fn conversation_by_agent_stores_values() {
        let by_agent = ConversationByAgent {
            agent_name: "support-bot".to_string(),
            conversation_count: 500,
            avg_messages: 12.5,
            success_rate: 0.92,
        };

        assert_eq!(by_agent.agent_name, "support-bot");
        assert_eq!(by_agent.conversation_count, 500);
        assert!((by_agent.avg_messages - 12.5).abs() < f64::EPSILON);
        assert!((by_agent.success_rate - 0.92).abs() < f64::EPSILON);
    }
}

mod recent_conversation_tests {
    use super::*;

    #[test]
    fn recent_conversation_stores_values() {
        let now = Utc::now();
        let conv = RecentConversation {
            context_id: ContextId::new("ctx_123".to_string()),
            agent_name: "assistant".to_string(),
            user_name: "John".to_string(),
            status: "active".to_string(),
            message_count: 10,
            started_at: now,
        };

        assert_eq!(conv.context_id.as_str(), "ctx_123");
        assert_eq!(conv.agent_name, "assistant");
        assert_eq!(conv.user_name, "John");
        assert_eq!(conv.status, "active");
        assert_eq!(conv.message_count, 10);
    }
}

mod content_stat_tests {
    use super::*;

    #[test]
    fn content_stat_stores_values() {
        let stat = ContentStat {
            title: "Getting Started".to_string(),
            slug: "getting-started".to_string(),
            views_5m: 10,
            views_1h: 50,
            views_1d: 500,
            views_7d: 2500,
            views_30d: 10000,
        };

        assert_eq!(stat.title, "Getting Started");
        assert_eq!(stat.slug, "getting-started");
        assert_eq!(stat.views_5m, 10);
        assert_eq!(stat.views_1h, 50);
        assert_eq!(stat.views_1d, 500);
        assert_eq!(stat.views_7d, 2500);
        assert_eq!(stat.views_30d, 10000);
    }

    #[test]
    fn content_stat_serializes() {
        let stat = ContentStat {
            title: "API Guide".to_string(),
            slug: "api-guide".to_string(),
            views_5m: 5,
            views_1h: 25,
            views_1d: 250,
            views_7d: 1250,
            views_30d: 5000,
        };
        let json = serde_json::to_string(&stat).unwrap();

        assert!(json.contains("API Guide"));
        assert!(json.contains("api-guide"));
        assert!(json.contains("views_5m"));
    }
}

mod error_summary_tests {
    use super::*;

    #[test]
    fn error_summary_stores_values() {
        let now = Utc::now();
        let summary = ErrorSummary {
            error_type: "database_connection".to_string(),
            count: 50,
            last_occurred: now,
            sample_message: Some("Connection timeout".to_string()),
        };

        assert_eq!(summary.error_type, "database_connection");
        assert_eq!(summary.count, 50);
        assert_eq!(summary.last_occurred, now);
        assert_eq!(
            summary.sample_message,
            Some("Connection timeout".to_string())
        );
    }

    #[test]
    fn error_summary_without_message() {
        let summary = ErrorSummary {
            error_type: "unknown".to_string(),
            count: 10,
            last_occurred: Utc::now(),
            sample_message: None,
        };

        assert!(summary.sample_message.is_none());
    }
}

mod activity_trend_tests {
    use super::*;

    #[test]
    fn activity_trend_stores_values() {
        let now = Utc::now();
        let trend = ActivityTrend {
            date: now,
            sessions: 1000,
            contexts: 500,
            tasks: 2000,
            ai_requests: 5000,
            tool_executions: 3000,
        };

        assert_eq!(trend.date, now);
        assert_eq!(trend.sessions, 1000);
        assert_eq!(trend.contexts, 500);
        assert_eq!(trend.tasks, 2000);
        assert_eq!(trend.ai_requests, 5000);
        assert_eq!(trend.tool_executions, 3000);
    }

    #[test]
    fn activity_trend_serializes() {
        let trend = ActivityTrend {
            date: Utc::now(),
            sessions: 10,
            contexts: 5,
            tasks: 20,
            ai_requests: 50,
            tool_executions: 30,
        };
        let json = serde_json::to_string(&trend).unwrap();

        assert!(json.contains("sessions"));
        assert!(json.contains("ai_requests"));
        assert!(json.contains("tool_executions"));
    }
}
