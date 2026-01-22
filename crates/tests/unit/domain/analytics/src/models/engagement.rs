//! Tests for engagement model types.

use systemprompt_analytics::{CreateEngagementEventInput, EngagementOptionalMetrics};

mod engagement_optional_metrics_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let metrics = EngagementOptionalMetrics::default();

        assert!(metrics.time_to_first_interaction_ms.is_none());
        assert!(metrics.time_to_first_scroll_ms.is_none());
        assert!(metrics.scroll_velocity_avg.is_none());
        assert!(metrics.scroll_direction_changes.is_none());
        assert!(metrics.mouse_move_distance_px.is_none());
        assert!(metrics.keyboard_events.is_none());
        assert!(metrics.copy_events.is_none());
        assert!(metrics.focus_time_ms.is_none());
        assert!(metrics.blur_count.is_none());
        assert!(metrics.visible_time_ms.is_none());
        assert!(metrics.hidden_time_ms.is_none());
        assert!(metrics.is_rage_click.is_none());
        assert!(metrics.is_dead_click.is_none());
        assert!(metrics.reading_pattern.is_none());
    }

    #[test]
    fn metrics_is_clone() {
        let metrics = EngagementOptionalMetrics {
            time_to_first_interaction_ms: Some(100),
            scroll_velocity_avg: Some(1.5),
            is_rage_click: Some(true),
            ..Default::default()
        };
        let cloned = metrics.clone();

        assert_eq!(
            metrics.time_to_first_interaction_ms,
            cloned.time_to_first_interaction_ms
        );
        assert_eq!(metrics.scroll_velocity_avg, cloned.scroll_velocity_avg);
        assert_eq!(metrics.is_rage_click, cloned.is_rage_click);
    }

    #[test]
    fn metrics_is_debug() {
        let metrics = EngagementOptionalMetrics::default();
        let debug_str = format!("{:?}", metrics);

        assert!(debug_str.contains("EngagementOptionalMetrics"));
    }

    #[test]
    fn metrics_deserializes_from_json() {
        let json = r#"{
            "time_to_first_interaction_ms": 500,
            "scroll_velocity_avg": 2.5,
            "is_rage_click": false,
            "reading_pattern": "scanning"
        }"#;

        let metrics: EngagementOptionalMetrics = serde_json::from_str(json).unwrap();

        assert_eq!(metrics.time_to_first_interaction_ms, Some(500));
        assert!((metrics.scroll_velocity_avg.unwrap() - 2.5).abs() < f32::EPSILON);
        assert_eq!(metrics.is_rage_click, Some(false));
        assert_eq!(metrics.reading_pattern, Some("scanning".to_string()));
    }

    #[test]
    fn metrics_deserializes_partial_json() {
        let json = r#"{"keyboard_events": 42}"#;

        let metrics: EngagementOptionalMetrics = serde_json::from_str(json).unwrap();

        assert_eq!(metrics.keyboard_events, Some(42));
        assert!(metrics.time_to_first_interaction_ms.is_none());
        assert!(metrics.scroll_velocity_avg.is_none());
    }

    #[test]
    fn metrics_deserializes_empty_json() {
        let json = r#"{}"#;

        let metrics: EngagementOptionalMetrics = serde_json::from_str(json).unwrap();

        assert!(metrics.time_to_first_interaction_ms.is_none());
        assert!(metrics.is_rage_click.is_none());
    }
}

mod create_engagement_event_input_tests {
    use super::*;

    fn create_input(page_url: &str, time_ms: i32, scroll: i32, clicks: i32) -> CreateEngagementEventInput {
        CreateEngagementEventInput {
            page_url: page_url.to_string(),
            time_on_page_ms: time_ms,
            max_scroll_depth: scroll,
            click_count: clicks,
            optional_metrics: EngagementOptionalMetrics::default(),
        }
    }

    #[test]
    fn input_stores_required_fields() {
        let input = create_input("/about", 5000, 75, 3);

        assert_eq!(input.page_url, "/about");
        assert_eq!(input.time_on_page_ms, 5000);
        assert_eq!(input.max_scroll_depth, 75);
        assert_eq!(input.click_count, 3);
    }

    #[test]
    fn input_with_optional_metrics() {
        let input = CreateEngagementEventInput {
            page_url: "/product".to_string(),
            time_on_page_ms: 10000,
            max_scroll_depth: 100,
            click_count: 5,
            optional_metrics: EngagementOptionalMetrics {
                time_to_first_interaction_ms: Some(200),
                is_dead_click: Some(true),
                ..Default::default()
            },
        };

        assert_eq!(input.optional_metrics.time_to_first_interaction_ms, Some(200));
        assert_eq!(input.optional_metrics.is_dead_click, Some(true));
        assert!(input.optional_metrics.is_rage_click.is_none());
    }

    #[test]
    fn input_is_clone() {
        let input = create_input("/page", 1000, 50, 2);
        let cloned = input.clone();

        assert_eq!(input.page_url, cloned.page_url);
        assert_eq!(input.time_on_page_ms, cloned.time_on_page_ms);
        assert_eq!(input.max_scroll_depth, cloned.max_scroll_depth);
        assert_eq!(input.click_count, cloned.click_count);
    }

    #[test]
    fn input_is_debug() {
        let input = create_input("/test", 500, 25, 1);
        let debug_str = format!("{:?}", input);

        assert!(debug_str.contains("CreateEngagementEventInput"));
        assert!(debug_str.contains("/test"));
    }

    #[test]
    fn input_deserializes_from_json() {
        let json = r#"{
            "page_url": "/checkout",
            "time_on_page_ms": 30000,
            "max_scroll_depth": 100,
            "click_count": 10,
            "is_rage_click": true,
            "reading_pattern": "focused"
        }"#;

        let input: CreateEngagementEventInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.page_url, "/checkout");
        assert_eq!(input.time_on_page_ms, 30000);
        assert_eq!(input.max_scroll_depth, 100);
        assert_eq!(input.click_count, 10);
        assert_eq!(input.optional_metrics.is_rage_click, Some(true));
        assert_eq!(input.optional_metrics.reading_pattern, Some("focused".to_string()));
    }

    #[test]
    fn input_deserializes_minimal_json() {
        let json = r#"{
            "page_url": "/home",
            "time_on_page_ms": 1000,
            "max_scroll_depth": 0,
            "click_count": 0
        }"#;

        let input: CreateEngagementEventInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.page_url, "/home");
        assert_eq!(input.time_on_page_ms, 1000);
        assert!(input.optional_metrics.is_rage_click.is_none());
    }

    #[test]
    fn input_handles_zero_values() {
        let input = create_input("/", 0, 0, 0);

        assert_eq!(input.time_on_page_ms, 0);
        assert_eq!(input.max_scroll_depth, 0);
        assert_eq!(input.click_count, 0);
    }

    #[test]
    fn input_handles_large_values() {
        let input = create_input("/long-session", i32::MAX, 100, 1000);

        assert_eq!(input.time_on_page_ms, i32::MAX);
        assert_eq!(input.max_scroll_depth, 100);
        assert_eq!(input.click_count, 1000);
    }
}
