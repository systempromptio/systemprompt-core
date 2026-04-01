//! Tests for funnel stats and funnel-with-steps types.

use systemprompt_analytics::{FunnelMatchType, FunnelStats, FunnelStepStats, FunnelWithSteps};
use systemprompt_identifiers::FunnelId;

mod funnel_stats_tests {
    use super::*;

    fn create_funnel_stats(
        funnel_id: &str,
        name: &str,
        entries: i64,
        completions: i64,
        rate: f64,
    ) -> FunnelStats {
        FunnelStats {
            funnel_id: FunnelId::new(funnel_id.to_string()),
            funnel_name: name.to_string(),
            total_entries: entries,
            total_completions: completions,
            overall_conversion_rate: rate,
            step_stats: vec![],
        }
    }

    #[test]
    fn stats_stores_funnel_id() {
        let stats = create_funnel_stats("fnl_123", "Test", 100, 50, 0.5);
        assert_eq!(stats.funnel_id.as_str(), "fnl_123");
    }

    #[test]
    fn stats_stores_funnel_name() {
        let stats = create_funnel_stats("fnl_456", "Purchase Funnel", 200, 50, 0.25);
        assert_eq!(stats.funnel_name, "Purchase Funnel");
    }

    #[test]
    fn stats_stores_total_entries() {
        let stats = create_funnel_stats("fnl_1", "Test", 1000, 100, 0.1);
        assert_eq!(stats.total_entries, 1000);
    }

    #[test]
    fn stats_stores_total_completions() {
        let stats = create_funnel_stats("fnl_2", "Test", 500, 250, 0.5);
        assert_eq!(stats.total_completions, 250);
    }

    #[test]
    fn stats_stores_overall_conversion_rate() {
        let stats = create_funnel_stats("fnl_3", "Test", 400, 100, 0.25);
        assert!((stats.overall_conversion_rate - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_with_step_stats() {
        let step_stats = vec![
            FunnelStepStats {
                step_order: 0,
                entered_count: 1000,
                exited_count: 800,
                conversion_rate: 0.8,
                avg_time_to_next_ms: Some(5000),
            },
            FunnelStepStats {
                step_order: 1,
                entered_count: 800,
                exited_count: 500,
                conversion_rate: 0.625,
                avg_time_to_next_ms: Some(10000),
            },
            FunnelStepStats {
                step_order: 2,
                entered_count: 500,
                exited_count: 500,
                conversion_rate: 1.0,
                avg_time_to_next_ms: None,
            },
        ];

        let stats = FunnelStats {
            funnel_id: FunnelId::new("fnl_full".to_string()),
            funnel_name: "Full Funnel".to_string(),
            total_entries: 1000,
            total_completions: 500,
            overall_conversion_rate: 0.5,
            step_stats,
        };

        assert_eq!(stats.step_stats.len(), 3);
        assert_eq!(stats.step_stats[0].entered_count, 1000);
        assert_eq!(stats.step_stats[2].step_order, 2);
    }

    #[test]
    fn stats_is_debug() {
        let stats = create_funnel_stats("fnl_debug", "Debug Test", 50, 25, 0.5);
        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("FunnelStats"));
    }

    #[test]
    fn stats_serializes() {
        let stats = create_funnel_stats("fnl_ser", "Serialize", 100, 75, 0.75);
        let json = serde_json::to_string(&stats).unwrap();

        assert!(json.contains("funnel_id"));
        assert!(json.contains("fnl_ser"));
        assert!(json.contains("Serialize"));
        assert!(json.contains("overall_conversion_rate"));
    }

    #[test]
    fn stats_deserializes() {
        let json = r#"{
            "funnel_id": "fnl_deser",
            "funnel_name": "Deserialized",
            "total_entries": 1000,
            "total_completions": 250,
            "overall_conversion_rate": 0.25,
            "step_stats": []
        }"#;

        let stats: FunnelStats = serde_json::from_str(json).unwrap();

        assert_eq!(stats.funnel_id.as_str(), "fnl_deser");
        assert_eq!(stats.funnel_name, "Deserialized");
        assert_eq!(stats.total_entries, 1000);
        assert_eq!(stats.total_completions, 250);
        assert!(stats.step_stats.is_empty());
    }

    #[test]
    fn stats_handles_zero_entries() {
        let stats = create_funnel_stats("fnl_empty", "Empty", 0, 0, 0.0);

        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_completions, 0);
        assert!((stats.overall_conversion_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_handles_perfect_conversion() {
        let stats = create_funnel_stats("fnl_perfect", "Perfect", 100, 100, 1.0);

        assert_eq!(stats.total_entries, 100);
        assert_eq!(stats.total_completions, 100);
        assert!((stats.overall_conversion_rate - 1.0).abs() < f64::EPSILON);
    }
}

mod funnel_with_steps_tests {
    use super::*;
    use chrono::Utc;
    use systemprompt_analytics::{Funnel, FunnelStep};

    fn create_funnel_with_steps() -> FunnelWithSteps {
        let now = Utc::now();
        let funnel = Funnel {
            id: FunnelId::new("fnl_test".to_string()),
            name: "Test Funnel".to_string(),
            description: Some("A test funnel".to_string()),
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        let steps = vec![
            FunnelStep {
                funnel_id: FunnelId::new("fnl_test".to_string()),
                step_order: 0,
                name: "Step 1".to_string(),
                match_pattern: "/step1".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
            FunnelStep {
                funnel_id: FunnelId::new("fnl_test".to_string()),
                step_order: 1,
                name: "Step 2".to_string(),
                match_pattern: "/step2".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
        ];

        FunnelWithSteps { funnel, steps }
    }

    #[test]
    fn funnel_with_steps_stores_funnel() {
        let fws = create_funnel_with_steps();
        assert_eq!(fws.funnel.name, "Test Funnel");
        assert!(fws.funnel.is_active);
    }

    #[test]
    fn funnel_with_steps_stores_steps() {
        let fws = create_funnel_with_steps();
        assert_eq!(fws.steps.len(), 2);
        assert_eq!(fws.steps[0].name, "Step 1");
        assert_eq!(fws.steps[1].step_order, 1);
    }

    #[test]
    fn funnel_with_steps_is_debug() {
        let fws = create_funnel_with_steps();
        let debug_str = format!("{:?}", fws);
        assert!(debug_str.contains("FunnelWithSteps"));
    }

    #[test]
    fn funnel_with_steps_serializes() {
        let fws = create_funnel_with_steps();
        let json = serde_json::to_string(&fws).unwrap();

        assert!(json.contains("Test Funnel"));
        assert!(json.contains("Step 1"));
        assert!(json.contains("Step 2"));
    }
}
