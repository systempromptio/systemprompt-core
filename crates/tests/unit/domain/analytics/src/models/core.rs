//! Tests for core analytics model types from mod.rs

use chrono::{Duration, Utc};
use systemprompt_analytics::{
    ActivityTrend, AnalyticsEvent, AnalyticsSession, BotTrafficStats, BrowserBreakdown,
    ContentStat, ConversationByAgent, ConversationSummary, ConversationTrend, CostOverview,
    DeviceBreakdown, ErrorSummary, GeographicBreakdown, PlatformOverview, RecentConversation,
    TopAgent, TopTool, TopUser, TrafficSource, TrafficSummary, UserMetricsWithTrends,
};
use systemprompt_identifiers::{ContextId, SessionId, UserId};

mod user_metrics_with_trends_tests {
    use super::*;

    fn create_metrics(
        count_24h: i64,
        count_7d: i64,
        count_30d: i64,
        prev_24h: i64,
        prev_7d: i64,
        prev_30d: i64,
    ) -> UserMetricsWithTrends {
        UserMetricsWithTrends {
            count_24h,
            count_7d,
            count_30d,
            prev_24h,
            prev_7d,
            prev_30d,
        }
    }

    #[test]
    fn metrics_stores_24h_values() {
        let metrics = create_metrics(100, 500, 2000, 90, 480, 1900);
        assert_eq!(metrics.count_24h, 100);
        assert_eq!(metrics.prev_24h, 90);
    }

    #[test]
    fn metrics_stores_7d_values() {
        let metrics = create_metrics(100, 500, 2000, 90, 480, 1900);
        assert_eq!(metrics.count_7d, 500);
        assert_eq!(metrics.prev_7d, 480);
    }

    #[test]
    fn metrics_stores_30d_values() {
        let metrics = create_metrics(100, 500, 2000, 90, 480, 1900);
        assert_eq!(metrics.count_30d, 2000);
        assert_eq!(metrics.prev_30d, 1900);
    }

    #[test]
    fn metrics_is_copy() {
        let metrics = create_metrics(10, 50, 200, 8, 45, 180);
        let copied = metrics;
        assert_eq!(metrics.count_24h, copied.count_24h);
    }

    #[test]
    fn metrics_is_clone() {
        let metrics = create_metrics(10, 50, 200, 8, 45, 180);
        let cloned = metrics.clone();
        assert_eq!(metrics.count_7d, cloned.count_7d);
    }

    #[test]
    fn metrics_is_debug() {
        let metrics = create_metrics(1, 2, 3, 0, 1, 2);
        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("UserMetricsWithTrends"));
    }

    #[test]
    fn metrics_serializes_with_renamed_fields() {
        let metrics = create_metrics(100, 500, 2000, 90, 480, 1900);
        let json = serde_json::to_string(&metrics).unwrap();

        assert!(json.contains("users_24h"));
        assert!(json.contains("users_7d"));
        assert!(json.contains("users_30d"));
        assert!(json.contains("users_prev_24h"));
    }

    #[test]
    fn metrics_deserializes() {
        let json = r#"{
            "users_24h": 150,
            "users_7d": 700,
            "users_30d": 2500,
            "users_prev_24h": 140,
            "users_prev_7d": 680,
            "users_prev_30d": 2400
        }"#;

        let metrics: UserMetricsWithTrends = serde_json::from_str(json).unwrap();

        assert_eq!(metrics.count_24h, 150);
        assert_eq!(metrics.count_7d, 700);
        assert_eq!(metrics.prev_30d, 2400);
    }
}

mod analytics_session_tests {
    use super::*;

    fn create_session() -> AnalyticsSession {
        let now = Utc::now();
        AnalyticsSession {
            session_id: SessionId::new("sess_123".to_string()),
            user_id: Some(UserId::new("user_456".to_string())),
            fingerprint_hash: Some("fp_abc".to_string()),
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Mozilla/5.0 Chrome/120".to_string()),
            device_type: Some("desktop".to_string()),
            browser: Some("Chrome".to_string()),
            os: Some("Windows".to_string()),
            country: Some("US".to_string()),
            city: Some("New York".to_string()),
            referrer_url: Some("https://google.com".to_string()),
            utm_source: Some("google".to_string()),
            utm_medium: Some("cpc".to_string()),
            utm_campaign: Some("summer_sale".to_string()),
            is_bot: false,
            is_scanner: Some(false),
            is_behavioral_bot: Some(false),
            behavioral_bot_reason: None,
            started_at: Some(now - Duration::hours(1)),
            last_activity_at: Some(now),
            ended_at: None,
            request_count: Some(25),
            task_count: Some(5),
            ai_request_count: Some(10),
            message_count: Some(15),
        }
    }

    #[test]
    fn session_stores_session_id() {
        let session = create_session();
        assert_eq!(session.session_id.as_str(), "sess_123");
    }

    #[test]
    fn session_stores_user_id() {
        let session = create_session();
        assert!(session.user_id.is_some());
        assert_eq!(session.user_id.unwrap().as_str(), "user_456");
    }

    #[test]
    fn session_stores_fingerprint() {
        let session = create_session();
        assert_eq!(session.fingerprint_hash, Some("fp_abc".to_string()));
    }

    #[test]
    fn session_stores_device_info() {
        let session = create_session();
        assert_eq!(session.device_type, Some("desktop".to_string()));
        assert_eq!(session.browser, Some("Chrome".to_string()));
        assert_eq!(session.os, Some("Windows".to_string()));
    }

    #[test]
    fn session_stores_location() {
        let session = create_session();
        assert_eq!(session.country, Some("US".to_string()));
        assert_eq!(session.city, Some("New York".to_string()));
    }

    #[test]
    fn session_stores_utm_params() {
        let session = create_session();
        assert_eq!(session.utm_source, Some("google".to_string()));
        assert_eq!(session.utm_medium, Some("cpc".to_string()));
        assert_eq!(session.utm_campaign, Some("summer_sale".to_string()));
    }

    #[test]
    fn session_stores_bot_flags() {
        let session = create_session();
        assert!(!session.is_bot);
        assert_eq!(session.is_scanner, Some(false));
        assert_eq!(session.is_behavioral_bot, Some(false));
    }

    #[test]
    fn session_stores_activity_counts() {
        let session = create_session();
        assert_eq!(session.request_count, Some(25));
        assert_eq!(session.task_count, Some(5));
        assert_eq!(session.ai_request_count, Some(10));
        assert_eq!(session.message_count, Some(15));
    }

    #[test]
    fn session_is_clone() {
        let session = create_session();
        let cloned = session.clone();

        assert_eq!(session.session_id.as_str(), cloned.session_id.as_str());
        assert_eq!(session.browser, cloned.browser);
    }

    #[test]
    fn session_is_debug() {
        let session = create_session();
        let debug_str = format!("{:?}", session);
        assert!(debug_str.contains("AnalyticsSession"));
    }

    #[test]
    fn session_serializes() {
        let session = create_session();
        let json = serde_json::to_string(&session).unwrap();

        assert!(json.contains("sess_123"));
        assert!(json.contains("Chrome"));
        assert!(json.contains("google"));
    }

    #[test]
    fn session_with_minimal_data() {
        let session = AnalyticsSession {
            session_id: SessionId::new("sess_min".to_string()),
            user_id: None,
            fingerprint_hash: None,
            ip_address: None,
            user_agent: None,
            device_type: None,
            browser: None,
            os: None,
            country: None,
            city: None,
            referrer_url: None,
            utm_source: None,
            utm_medium: None,
            utm_campaign: None,
            is_bot: true,
            is_scanner: None,
            is_behavioral_bot: None,
            behavioral_bot_reason: None,
            started_at: None,
            last_activity_at: None,
            ended_at: None,
            request_count: None,
            task_count: None,
            ai_request_count: None,
            message_count: None,
        };

        assert_eq!(session.session_id.as_str(), "sess_min");
        assert!(session.user_id.is_none());
        assert!(session.is_bot);
    }
}

mod analytics_event_tests {
    use super::*;

    fn create_event() -> AnalyticsEvent {
        AnalyticsEvent {
            id: "evt_123".to_string(),
            event_type: "page_view".to_string(),
            event_category: "navigation".to_string(),
            severity: "info".to_string(),
            user_id: UserId::new("user_456".to_string()),
            session_id: Some(SessionId::new("sess_789".to_string())),
            message: Some("User viewed homepage".to_string()),
            metadata: Some(r#"{"page": "/home"}"#.to_string()),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn event_stores_id() {
        let event = create_event();
        assert_eq!(event.id, "evt_123");
    }

    #[test]
    fn event_stores_event_type() {
        let event = create_event();
        assert_eq!(event.event_type, "page_view");
    }

    #[test]
    fn event_stores_category() {
        let event = create_event();
        assert_eq!(event.event_category, "navigation");
    }

    #[test]
    fn event_stores_severity() {
        let event = create_event();
        assert_eq!(event.severity, "info");
    }

    #[test]
    fn event_stores_user_id() {
        let event = create_event();
        assert_eq!(event.user_id.as_str(), "user_456");
    }

    #[test]
    fn event_stores_session_id() {
        let event = create_event();
        assert!(event.session_id.is_some());
        assert_eq!(event.session_id.unwrap().as_str(), "sess_789");
    }

    #[test]
    fn event_stores_message() {
        let event = create_event();
        assert_eq!(event.message, Some("User viewed homepage".to_string()));
    }

    #[test]
    fn event_stores_metadata() {
        let event = create_event();
        assert!(event.metadata.is_some());
        assert!(event.metadata.unwrap().contains("page"));
    }

    #[test]
    fn event_is_clone() {
        let event = create_event();
        let cloned = event.clone();

        assert_eq!(event.id, cloned.id);
        assert_eq!(event.event_type, cloned.event_type);
    }

    #[test]
    fn event_is_debug() {
        let event = create_event();
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("AnalyticsEvent"));
    }

    #[test]
    fn event_serializes() {
        let event = create_event();
        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("evt_123"));
        assert!(json.contains("page_view"));
    }
}

mod platform_overview_tests {
    use super::*;

    fn create_overview() -> PlatformOverview {
        PlatformOverview {
            total_users: 10000,
            active_users_24h: 500,
            active_users_7d: 2000,
            total_sessions: 50000,
            active_sessions: 100,
            total_contexts: 15000,
            total_tasks: 75000,
            total_ai_requests: 100000,
        }
    }

    #[test]
    fn overview_stores_user_counts() {
        let overview = create_overview();
        assert_eq!(overview.total_users, 10000);
        assert_eq!(overview.active_users_24h, 500);
        assert_eq!(overview.active_users_7d, 2000);
    }

    #[test]
    fn overview_stores_session_counts() {
        let overview = create_overview();
        assert_eq!(overview.total_sessions, 50000);
        assert_eq!(overview.active_sessions, 100);
    }

    #[test]
    fn overview_stores_activity_counts() {
        let overview = create_overview();
        assert_eq!(overview.total_contexts, 15000);
        assert_eq!(overview.total_tasks, 75000);
        assert_eq!(overview.total_ai_requests, 100000);
    }

    #[test]
    fn overview_is_copy() {
        let overview = create_overview();
        let copied = overview;
        assert_eq!(overview.total_users, copied.total_users);
    }

    #[test]
    fn overview_is_clone() {
        let overview = create_overview();
        let cloned = overview.clone();
        assert_eq!(overview.total_tasks, cloned.total_tasks);
    }

    #[test]
    fn overview_is_debug() {
        let overview = create_overview();
        let debug_str = format!("{:?}", overview);
        assert!(debug_str.contains("PlatformOverview"));
    }

    #[test]
    fn overview_serializes() {
        let overview = create_overview();
        let json = serde_json::to_string(&overview).unwrap();

        assert!(json.contains("total_users"));
        assert!(json.contains("active_sessions"));
    }

    #[test]
    fn overview_deserializes() {
        let json = r#"{
            "total_users": 5000,
            "active_users_24h": 250,
            "active_users_7d": 1000,
            "total_sessions": 25000,
            "active_sessions": 50,
            "total_contexts": 7500,
            "total_tasks": 37500,
            "total_ai_requests": 50000
        }"#;

        let overview: PlatformOverview = serde_json::from_str(json).unwrap();

        assert_eq!(overview.total_users, 5000);
        assert_eq!(overview.active_sessions, 50);
    }
}

mod cost_overview_tests {
    use super::*;

    fn create_cost_overview() -> CostOverview {
        CostOverview {
            total_cost: 1000.50,
            cost_24h: 50.25,
            cost_7d: 200.75,
            cost_30d: 800.00,
            avg_cost_per_request: 0.01,
        }
    }

    #[test]
    fn cost_stores_totals() {
        let cost = create_cost_overview();
        assert!((cost.total_cost - 1000.50).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_stores_period_costs() {
        let cost = create_cost_overview();
        assert!((cost.cost_24h - 50.25).abs() < f64::EPSILON);
        assert!((cost.cost_7d - 200.75).abs() < f64::EPSILON);
        assert!((cost.cost_30d - 800.00).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_stores_average() {
        let cost = create_cost_overview();
        assert!((cost.avg_cost_per_request - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_is_copy() {
        let cost = create_cost_overview();
        let copied = cost;
        assert!((cost.total_cost - copied.total_cost).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_is_clone() {
        let cost = create_cost_overview();
        let cloned = cost.clone();
        assert!((cost.cost_7d - cloned.cost_7d).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_is_debug() {
        let cost = create_cost_overview();
        let debug_str = format!("{:?}", cost);
        assert!(debug_str.contains("CostOverview"));
    }

    #[test]
    fn cost_serializes() {
        let cost = create_cost_overview();
        let json = serde_json::to_string(&cost).unwrap();

        assert!(json.contains("total_cost"));
        assert!(json.contains("avg_cost_per_request"));
    }

    #[test]
    fn cost_deserializes() {
        let json = r#"{
            "total_cost": 500.0,
            "cost_24h": 25.0,
            "cost_7d": 100.0,
            "cost_30d": 400.0,
            "avg_cost_per_request": 0.005
        }"#;

        let cost: CostOverview = serde_json::from_str(json).unwrap();

        assert!((cost.total_cost - 500.0).abs() < f64::EPSILON);
        assert!((cost.avg_cost_per_request - 0.005).abs() < f64::EPSILON);
    }
}

mod traffic_stats_tests {
    use super::*;

    #[test]
    fn traffic_summary_stores_values() {
        let summary = TrafficSummary {
            total_sessions: 10000,
            unique_visitors: 8000,
            page_views: 50000,
            avg_session_duration_seconds: 180.5,
            bounce_rate: 0.35,
        };

        assert_eq!(summary.total_sessions, 10000);
        assert_eq!(summary.unique_visitors, 8000);
        assert_eq!(summary.page_views, 50000);
        assert!((summary.avg_session_duration_seconds - 180.5).abs() < f64::EPSILON);
        assert!((summary.bounce_rate - 0.35).abs() < f64::EPSILON);
    }

    #[test]
    fn traffic_summary_is_copy() {
        let summary = TrafficSummary {
            total_sessions: 100,
            unique_visitors: 80,
            page_views: 500,
            avg_session_duration_seconds: 120.0,
            bounce_rate: 0.4,
        };
        let copied = summary;
        assert_eq!(summary.total_sessions, copied.total_sessions);
    }

    #[test]
    fn traffic_source_stores_values() {
        let source = TrafficSource {
            source: "google".to_string(),
            sessions: 5000,
            percentage: 0.5,
        };

        assert_eq!(source.source, "google");
        assert_eq!(source.sessions, 5000);
        assert!((source.percentage - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn traffic_source_is_clone() {
        let source = TrafficSource {
            source: "direct".to_string(),
            sessions: 3000,
            percentage: 0.3,
        };
        let cloned = source.clone();
        assert_eq!(source.source, cloned.source);
    }
}

mod breakdown_tests {
    use super::*;

    #[test]
    fn device_breakdown_stores_values() {
        let breakdown = DeviceBreakdown {
            device_type: "desktop".to_string(),
            count: 6000,
            percentage: 0.6,
        };

        assert_eq!(breakdown.device_type, "desktop");
        assert_eq!(breakdown.count, 6000);
        assert!((breakdown.percentage - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn device_breakdown_is_clone() {
        let breakdown = DeviceBreakdown {
            device_type: "mobile".to_string(),
            count: 3500,
            percentage: 0.35,
        };
        let cloned = breakdown.clone();
        assert_eq!(breakdown.device_type, cloned.device_type);
    }

    #[test]
    fn browser_breakdown_stores_values() {
        let breakdown = BrowserBreakdown {
            browser: "Chrome".to_string(),
            count: 7000,
            percentage: 0.7,
        };

        assert_eq!(breakdown.browser, "Chrome");
        assert_eq!(breakdown.count, 7000);
    }

    #[test]
    fn geographic_breakdown_stores_values() {
        let breakdown = GeographicBreakdown {
            country: "United States".to_string(),
            count: 4000,
            percentage: 0.4,
        };

        assert_eq!(breakdown.country, "United States");
        assert_eq!(breakdown.count, 4000);
    }
}

mod bot_traffic_stats_tests {
    use super::*;

    #[test]
    fn default_creates_zeroed_stats() {
        let stats = BotTrafficStats::default();

        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.bot_requests, 0);
        assert_eq!(stats.human_requests, 0);
        assert!((stats.bot_percentage - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_stores_values() {
        let stats = BotTrafficStats {
            total_requests: 10000,
            bot_requests: 2000,
            human_requests: 8000,
            bot_percentage: 0.2,
        };

        assert_eq!(stats.total_requests, 10000);
        assert_eq!(stats.bot_requests, 2000);
        assert_eq!(stats.human_requests, 8000);
        assert!((stats.bot_percentage - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_is_copy() {
        let stats = BotTrafficStats {
            total_requests: 100,
            bot_requests: 10,
            human_requests: 90,
            bot_percentage: 0.1,
        };
        let copied = stats;
        assert_eq!(stats.total_requests, copied.total_requests);
    }

    #[test]
    fn stats_is_debug() {
        let stats = BotTrafficStats::default();
        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("BotTrafficStats"));
    }

    #[test]
    fn stats_serializes() {
        let stats = BotTrafficStats {
            total_requests: 1000,
            bot_requests: 100,
            human_requests: 900,
            bot_percentage: 0.1,
        };
        let json = serde_json::to_string(&stats).unwrap();

        assert!(json.contains("total_requests"));
        assert!(json.contains("bot_percentage"));
    }

    #[test]
    fn stats_deserializes() {
        let json = r#"{
            "total_requests": 5000,
            "bot_requests": 500,
            "human_requests": 4500,
            "bot_percentage": 0.1
        }"#;

        let stats: BotTrafficStats = serde_json::from_str(json).unwrap();

        assert_eq!(stats.total_requests, 5000);
        assert_eq!(stats.bot_requests, 500);
    }
}

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
    fn top_user_is_clone() {
        let user = TopUser {
            user_id: UserId::new("user_1".to_string()),
            user_name: "Test".to_string(),
            session_count: 10,
            task_count: 50,
            ai_request_count: 100,
            total_cost: 10.0,
        };
        let cloned = user.clone();
        assert_eq!(user.user_name, cloned.user_name);
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
    fn conversation_summary_is_copy() {
        let summary = ConversationSummary {
            total_conversations: 100,
            active_conversations: 10,
            completed_conversations: 90,
            avg_messages_per_conversation: 10.0,
            avg_duration_minutes: 5.0,
        };
        let copied = summary;
        assert_eq!(summary.total_conversations, copied.total_conversations);
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

    #[test]
    fn recent_conversation_is_clone() {
        let conv = RecentConversation {
            context_id: ContextId::new("ctx_clone".to_string()),
            agent_name: "test".to_string(),
            user_name: "test".to_string(),
            status: "completed".to_string(),
            message_count: 5,
            started_at: Utc::now(),
        };
        let cloned = conv.clone();
        assert_eq!(conv.agent_name, cloned.agent_name);
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
    fn content_stat_is_clone() {
        let stat = ContentStat {
            title: "Test".to_string(),
            slug: "test".to_string(),
            views_5m: 1,
            views_1h: 5,
            views_1d: 50,
            views_7d: 250,
            views_30d: 1000,
        };
        let cloned = stat.clone();
        assert_eq!(stat.title, cloned.title);
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
        assert_eq!(summary.sample_message, Some("Connection timeout".to_string()));
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

    #[test]
    fn error_summary_is_clone() {
        let summary = ErrorSummary {
            error_type: "test".to_string(),
            count: 5,
            last_occurred: Utc::now(),
            sample_message: Some("test".to_string()),
        };
        let cloned = summary.clone();
        assert_eq!(summary.error_type, cloned.error_type);
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
    fn activity_trend_is_copy() {
        let trend = ActivityTrend {
            date: Utc::now(),
            sessions: 100,
            contexts: 50,
            tasks: 200,
            ai_requests: 500,
            tool_executions: 300,
        };
        let copied = trend;
        assert_eq!(trend.sessions, copied.sessions);
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
