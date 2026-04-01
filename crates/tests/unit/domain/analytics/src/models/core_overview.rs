//! Tests for UserMetricsWithTrends, PlatformOverview, and CostOverview model types.

use systemprompt_analytics::{CostOverview, PlatformOverview, UserMetricsWithTrends};

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
