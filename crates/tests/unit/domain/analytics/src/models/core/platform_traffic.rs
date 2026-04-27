//! Tests for platform overview, cost, traffic, breakdown, and bot traffic model
//! types.

use systemprompt_analytics::{
    BotTrafficStats, BrowserBreakdown, CostOverview, DeviceBreakdown, GeographicBreakdown,
    PlatformOverview, TrafficSource, TrafficSummary,
};

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
}
