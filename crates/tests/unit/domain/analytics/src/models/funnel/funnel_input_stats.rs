//! Tests for funnel input and step stats types.

use systemprompt_analytics::{
    CreateFunnelInput, CreateFunnelStepInput, FunnelMatchType, FunnelStepStats,
};

mod create_funnel_input_tests {
    use super::*;

    fn create_funnel(
        name: &str,
        desc: Option<&str>,
        steps: Vec<CreateFunnelStepInput>,
    ) -> CreateFunnelInput {
        CreateFunnelInput {
            name: name.to_string(),
            description: desc.map(|s| s.to_string()),
            steps,
        }
    }

    fn sample_step() -> CreateFunnelStepInput {
        CreateFunnelStepInput {
            name: "Step 1".to_string(),
            match_pattern: "/step1".to_string(),
            match_type: FunnelMatchType::UrlExact,
        }
    }

    #[test]
    fn funnel_stores_name() {
        let funnel = create_funnel("Signup Funnel", None, vec![]);
        assert_eq!(funnel.name, "Signup Funnel");
    }

    #[test]
    fn funnel_stores_description() {
        let funnel = create_funnel("Test", Some("A test funnel"), vec![]);
        assert_eq!(funnel.description, Some("A test funnel".to_string()));
    }

    #[test]
    fn funnel_stores_steps() {
        let steps = vec![
            CreateFunnelStepInput {
                name: "Landing".to_string(),
                match_pattern: "/landing".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
            CreateFunnelStepInput {
                name: "Signup".to_string(),
                match_pattern: "/signup".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
            CreateFunnelStepInput {
                name: "Complete".to_string(),
                match_pattern: "/complete".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
        ];

        let funnel = create_funnel("Registration", None, steps);

        assert_eq!(funnel.steps.len(), 3);
        assert_eq!(funnel.steps[0].name, "Landing");
        assert_eq!(funnel.steps[2].name, "Complete");
    }

    #[test]
    fn funnel_with_no_description() {
        let funnel = create_funnel("Simple", None, vec![sample_step()]);
        assert!(funnel.description.is_none());
    }

    #[test]
    fn funnel_is_debug() {
        let funnel = create_funnel("Debug Test", None, vec![]);
        let debug_str = format!("{:?}", funnel);
        assert!(debug_str.contains("CreateFunnelInput"));
    }

    #[test]
    fn funnel_serializes() {
        let funnel = create_funnel("Serialize Test", Some("desc"), vec![sample_step()]);
        let json = serde_json::to_string(&funnel).unwrap();

        assert!(json.contains("Serialize Test"));
        assert!(json.contains("desc"));
        assert!(json.contains("steps"));
    }
}

mod funnel_step_stats_tests {
    use super::*;

    fn create_stats(
        step_order: i32,
        entered: i64,
        exited: i64,
        rate: f64,
        avg_time: Option<i64>,
    ) -> FunnelStepStats {
        FunnelStepStats {
            step_order,
            entered_count: entered,
            exited_count: exited,
            conversion_rate: rate,
            avg_time_to_next_ms: avg_time,
        }
    }

    #[test]
    fn stats_stores_step_order() {
        let stats = create_stats(1, 100, 90, 0.9, Some(5000));
        assert_eq!(stats.step_order, 1);
    }

    #[test]
    fn stats_stores_entered_count() {
        let stats = create_stats(0, 500, 400, 0.8, None);
        assert_eq!(stats.entered_count, 500);
    }

    #[test]
    fn stats_stores_exited_count() {
        let stats = create_stats(2, 100, 75, 0.75, Some(3000));
        assert_eq!(stats.exited_count, 75);
    }

    #[test]
    fn stats_stores_conversion_rate() {
        let stats = create_stats(1, 100, 50, 0.5, None);
        assert!((stats.conversion_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_stores_avg_time() {
        let stats = create_stats(1, 100, 90, 0.9, Some(10000));
        assert_eq!(stats.avg_time_to_next_ms, Some(10000));
    }

    #[test]
    fn stats_handles_no_avg_time() {
        let stats = create_stats(3, 50, 0, 0.0, None);
        assert!(stats.avg_time_to_next_ms.is_none());
    }

    #[test]
    fn stats_is_debug() {
        let stats = create_stats(0, 100, 100, 1.0, None);
        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("FunnelStepStats"));
    }

    #[test]
    fn stats_serializes() {
        let stats = create_stats(1, 100, 75, 0.75, Some(2500));
        let json = serde_json::to_string(&stats).unwrap();

        assert!(json.contains("step_order"));
        assert!(json.contains("entered_count"));
        assert!(json.contains("conversion_rate"));
    }
}
