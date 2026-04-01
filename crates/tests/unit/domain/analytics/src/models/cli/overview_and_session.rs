//! Tests for overview rows and session rows.

use chrono::Utc;
use systemprompt_analytics::models::{
    LiveSessionRow, OverviewAgentRow, OverviewCostRow, OverviewRequestRow, OverviewToolRow,
    SessionStatsRow, SessionTrendRow,
};
use systemprompt_identifiers::{SessionId, UserId};

mod overview_row_tests {
    use super::*;

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
        row.user_id.as_ref().expect("user_id should be present");
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
