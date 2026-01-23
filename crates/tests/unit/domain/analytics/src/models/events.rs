//! Tests for analytics event model types.

use systemprompt_analytics::{
    AnalyticsEventBatchResponse, AnalyticsEventCreated, AnalyticsEventType,
    ConversionEventData, CreateAnalyticsEventBatchInput, CreateAnalyticsEventInput,
    EngagementEventData, LinkClickEventData, ScrollEventData,
};

mod analytics_event_type_tests {
    use super::*;

    #[test]
    fn page_view_as_str() {
        assert_eq!(AnalyticsEventType::PageView.as_str(), "page_view");
    }

    #[test]
    fn page_exit_as_str() {
        assert_eq!(AnalyticsEventType::PageExit.as_str(), "page_exit");
    }

    #[test]
    fn link_click_as_str() {
        assert_eq!(AnalyticsEventType::LinkClick.as_str(), "link_click");
    }

    #[test]
    fn scroll_as_str() {
        assert_eq!(AnalyticsEventType::Scroll.as_str(), "scroll");
    }

    #[test]
    fn engagement_as_str() {
        assert_eq!(AnalyticsEventType::Engagement.as_str(), "engagement");
    }

    #[test]
    fn conversion_as_str() {
        assert_eq!(AnalyticsEventType::Conversion.as_str(), "conversion");
    }

    #[test]
    fn custom_as_str() {
        let custom = AnalyticsEventType::Custom("my_custom_event".to_string());
        assert_eq!(custom.as_str(), "my_custom_event");
    }

    #[test]
    fn page_view_category() {
        assert_eq!(AnalyticsEventType::PageView.category(), "navigation");
    }

    #[test]
    fn page_exit_category() {
        assert_eq!(AnalyticsEventType::PageExit.category(), "navigation");
    }

    #[test]
    fn link_click_category() {
        assert_eq!(AnalyticsEventType::LinkClick.category(), "interaction");
    }

    #[test]
    fn scroll_category() {
        assert_eq!(AnalyticsEventType::Scroll.category(), "engagement");
    }

    #[test]
    fn engagement_category() {
        assert_eq!(AnalyticsEventType::Engagement.category(), "engagement");
    }

    #[test]
    fn conversion_category() {
        assert_eq!(AnalyticsEventType::Conversion.category(), "conversion");
    }

    #[test]
    fn custom_category() {
        let custom = AnalyticsEventType::Custom("test".to_string());
        assert_eq!(custom.category(), "custom");
    }

    #[test]
    fn display_matches_as_str() {
        let types = [
            AnalyticsEventType::PageView,
            AnalyticsEventType::PageExit,
            AnalyticsEventType::LinkClick,
            AnalyticsEventType::Scroll,
            AnalyticsEventType::Engagement,
            AnalyticsEventType::Conversion,
            AnalyticsEventType::Custom("test".to_string()),
        ];

        for event_type in types {
            assert_eq!(format!("{}", event_type), event_type.as_str());
        }
    }

    #[test]
    fn event_type_is_eq() {
        assert_eq!(AnalyticsEventType::PageView, AnalyticsEventType::PageView);
        assert_ne!(AnalyticsEventType::PageView, AnalyticsEventType::PageExit);
    }

    #[test]
    fn custom_events_equality() {
        let custom1 = AnalyticsEventType::Custom("test".to_string());
        let custom2 = AnalyticsEventType::Custom("test".to_string());
        let custom3 = AnalyticsEventType::Custom("other".to_string());

        assert_eq!(custom1, custom2);
        assert_ne!(custom1, custom3);
    }

    #[test]
    fn event_type_serializes() {
        let event_type = AnalyticsEventType::PageView;
        let json = serde_json::to_string(&event_type).unwrap();
        assert!(json.contains("page_view"));
    }

    #[test]
    fn event_type_deserializes() {
        let json = r#""link_click""#;
        let event_type: AnalyticsEventType = serde_json::from_str(json).unwrap();
        assert_eq!(event_type, AnalyticsEventType::LinkClick);
    }

    #[test]
    fn custom_event_deserializes_unknown_type() {
        let json = r#""unknown_event_type""#;
        let event_type: AnalyticsEventType = serde_json::from_str(json).unwrap();
        assert_eq!(event_type, AnalyticsEventType::Custom("unknown_event_type".to_string()));
    }

    #[test]
    fn event_type_is_clone() {
        let event_type = AnalyticsEventType::Conversion;
        let cloned = event_type.clone();
        assert_eq!(event_type, cloned);
    }

    #[test]
    fn event_type_is_debug() {
        let debug_str = format!("{:?}", AnalyticsEventType::Scroll);
        assert!(debug_str.contains("Scroll"));
    }
}

mod engagement_event_data_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let data = EngagementEventData::default();

        assert!(data.scroll_depth.is_none());
        assert!(data.time_on_page_ms.is_none());
        assert!(data.time_to_first_interaction_ms.is_none());
        assert!(data.time_to_first_scroll_ms.is_none());
        assert!(data.click_count.is_none());
        assert!(data.mouse_move_distance_px.is_none());
        assert!(data.keyboard_events.is_none());
        assert!(data.copy_events.is_none());
        assert!(data.visible_time_ms.is_none());
        assert!(data.hidden_time_ms.is_none());
        assert!(data.is_rage_click.is_none());
        assert!(data.is_dead_click.is_none());
        assert!(data.reading_pattern.is_none());
    }

    #[test]
    fn data_with_values() {
        let data = EngagementEventData {
            scroll_depth: Some(75),
            time_on_page_ms: Some(30000),
            click_count: Some(5),
            is_rage_click: Some(true),
            reading_pattern: Some("scanning".to_string()),
            ..Default::default()
        };

        assert_eq!(data.scroll_depth, Some(75));
        assert_eq!(data.time_on_page_ms, Some(30000));
        assert_eq!(data.click_count, Some(5));
        assert_eq!(data.is_rage_click, Some(true));
        assert_eq!(data.reading_pattern, Some("scanning".to_string()));
    }

    #[test]
    fn data_serializes_skipping_none() {
        let data = EngagementEventData {
            scroll_depth: Some(50),
            ..Default::default()
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("scroll_depth"));
        assert!(!json.contains("time_on_page_ms"));
        assert!(!json.contains("is_rage_click"));
    }

    #[test]
    fn data_deserializes() {
        let json = r#"{"scroll_depth": 100, "is_dead_click": false}"#;
        let data: EngagementEventData = serde_json::from_str(json).unwrap();

        assert_eq!(data.scroll_depth, Some(100));
        assert_eq!(data.is_dead_click, Some(false));
        assert!(data.click_count.is_none());
    }

    #[test]
    fn data_is_clone() {
        let data = EngagementEventData {
            scroll_depth: Some(25),
            ..Default::default()
        };
        let cloned = data.clone();
        assert_eq!(data.scroll_depth, cloned.scroll_depth);
    }

    #[test]
    fn data_is_debug() {
        let data = EngagementEventData::default();
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("EngagementEventData"));
    }
}

mod link_click_event_data_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let data = LinkClickEventData::default();

        assert!(data.target_url.is_none());
        assert!(data.link_text.is_none());
        assert!(data.link_position.is_none());
        assert!(data.is_external.is_none());
    }

    #[test]
    fn data_with_values() {
        let data = LinkClickEventData {
            target_url: Some("https://example.com".to_string()),
            link_text: Some("Click here".to_string()),
            link_position: Some("header".to_string()),
            is_external: Some(true),
        };

        assert_eq!(data.target_url, Some("https://example.com".to_string()));
        assert_eq!(data.link_text, Some("Click here".to_string()));
        assert_eq!(data.link_position, Some("header".to_string()));
        assert_eq!(data.is_external, Some(true));
    }

    #[test]
    fn data_serializes_skipping_none() {
        let data = LinkClickEventData {
            target_url: Some("https://test.com".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("target_url"));
        assert!(!json.contains("link_text"));
    }

    #[test]
    fn data_deserializes() {
        let json = r#"{"target_url": "https://dest.com", "is_external": true}"#;
        let data: LinkClickEventData = serde_json::from_str(json).unwrap();

        assert_eq!(data.target_url, Some("https://dest.com".to_string()));
        assert_eq!(data.is_external, Some(true));
        assert!(data.link_text.is_none());
    }

    #[test]
    fn data_is_clone() {
        let data = LinkClickEventData {
            link_text: Some("Test".to_string()),
            ..Default::default()
        };
        let cloned = data.clone();
        assert_eq!(data.link_text, cloned.link_text);
    }

    #[test]
    fn data_is_debug() {
        let data = LinkClickEventData::default();
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("LinkClickEventData"));
    }
}

mod scroll_event_data_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let data = ScrollEventData::default();

        assert!(data.depth.is_none());
        assert!(data.milestone.is_none());
        assert!(data.direction.is_none());
        assert!(data.velocity.is_none());
    }

    #[test]
    fn data_with_values() {
        let data = ScrollEventData {
            depth: Some(50),
            milestone: Some(25),
            direction: Some("down".to_string()),
            velocity: Some(1.5),
        };

        assert_eq!(data.depth, Some(50));
        assert_eq!(data.milestone, Some(25));
        assert_eq!(data.direction, Some("down".to_string()));
        assert!((data.velocity.unwrap() - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn data_serializes_skipping_none() {
        let data = ScrollEventData {
            depth: Some(75),
            ..Default::default()
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("depth"));
        assert!(!json.contains("milestone"));
    }

    #[test]
    fn data_deserializes() {
        let json = r#"{"depth": 100, "direction": "up"}"#;
        let data: ScrollEventData = serde_json::from_str(json).unwrap();

        assert_eq!(data.depth, Some(100));
        assert_eq!(data.direction, Some("up".to_string()));
        assert!(data.velocity.is_none());
    }

    #[test]
    fn data_is_clone() {
        let data = ScrollEventData {
            milestone: Some(50),
            ..Default::default()
        };
        let cloned = data.clone();
        assert_eq!(data.milestone, cloned.milestone);
    }

    #[test]
    fn data_is_debug() {
        let data = ScrollEventData::default();
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("ScrollEventData"));
    }
}

mod conversion_event_data_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let data = ConversionEventData::default();

        assert!(data.goal_name.is_none());
        assert!(data.goal_value.is_none());
        assert!(data.funnel_step.is_none());
    }

    #[test]
    fn data_with_values() {
        let data = ConversionEventData {
            goal_name: Some("signup".to_string()),
            goal_value: Some(99.99),
            funnel_step: Some(3),
        };

        assert_eq!(data.goal_name, Some("signup".to_string()));
        assert!((data.goal_value.unwrap() - 99.99).abs() < f64::EPSILON);
        assert_eq!(data.funnel_step, Some(3));
    }

    #[test]
    fn data_serializes_skipping_none() {
        let data = ConversionEventData {
            goal_name: Some("purchase".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("goal_name"));
        assert!(!json.contains("goal_value"));
    }

    #[test]
    fn data_deserializes() {
        let json = r#"{"goal_name": "trial_start", "goal_value": 0.0}"#;
        let data: ConversionEventData = serde_json::from_str(json).unwrap();

        assert_eq!(data.goal_name, Some("trial_start".to_string()));
        assert!((data.goal_value.unwrap() - 0.0).abs() < f64::EPSILON);
        assert!(data.funnel_step.is_none());
    }

    #[test]
    fn data_is_clone() {
        let data = ConversionEventData {
            funnel_step: Some(1),
            ..Default::default()
        };
        let cloned = data.clone();
        assert_eq!(data.funnel_step, cloned.funnel_step);
    }

    #[test]
    fn data_is_debug() {
        let data = ConversionEventData::default();
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("ConversionEventData"));
    }
}

mod create_analytics_event_input_tests {
    use super::*;

    fn create_input(event_type: AnalyticsEventType, page_url: &str) -> CreateAnalyticsEventInput {
        CreateAnalyticsEventInput {
            event_type,
            page_url: page_url.to_string(),
            content_id: None,
            slug: None,
            referrer: None,
            data: None,
        }
    }

    #[test]
    fn input_stores_event_type() {
        let input = create_input(AnalyticsEventType::PageView, "/home");
        assert_eq!(input.event_type, AnalyticsEventType::PageView);
    }

    #[test]
    fn input_stores_page_url() {
        let input = create_input(AnalyticsEventType::Scroll, "/about");
        assert_eq!(input.page_url, "/about");
    }

    #[test]
    fn input_with_all_fields() {
        let data = serde_json::json!({"key": "value"});
        let input = CreateAnalyticsEventInput {
            event_type: AnalyticsEventType::Conversion,
            page_url: "/checkout".to_string(),
            content_id: Some(systemprompt_identifiers::ContentId::new("cnt_123".to_string())),
            slug: Some("checkout-page".to_string()),
            referrer: Some("https://google.com".to_string()),
            data: Some(data.clone()),
        };

        assert_eq!(input.event_type, AnalyticsEventType::Conversion);
        assert_eq!(input.page_url, "/checkout");
        assert!(input.content_id.is_some());
        assert_eq!(input.slug, Some("checkout-page".to_string()));
        assert_eq!(input.referrer, Some("https://google.com".to_string()));
        assert!(input.data.is_some());
    }

    #[test]
    fn input_is_clone() {
        let input = create_input(AnalyticsEventType::LinkClick, "/products");
        let cloned = input.clone();

        assert_eq!(input.event_type, cloned.event_type);
        assert_eq!(input.page_url, cloned.page_url);
    }

    #[test]
    fn input_is_debug() {
        let input = create_input(AnalyticsEventType::Engagement, "/test");
        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("CreateAnalyticsEventInput"));
    }

    #[test]
    fn input_deserializes_from_json() {
        let json = r#"{
            "event_type": "page_view",
            "page_url": "/landing",
            "referrer": "https://twitter.com"
        }"#;

        let input: CreateAnalyticsEventInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.event_type, AnalyticsEventType::PageView);
        assert_eq!(input.page_url, "/landing");
        assert_eq!(input.referrer, Some("https://twitter.com".to_string()));
        assert!(input.content_id.is_none());
    }

    #[test]
    fn input_deserializes_minimal_json() {
        let json = r#"{
            "event_type": "scroll",
            "page_url": "/blog/post-1"
        }"#;

        let input: CreateAnalyticsEventInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.event_type, AnalyticsEventType::Scroll);
        assert_eq!(input.page_url, "/blog/post-1");
        assert!(input.slug.is_none());
        assert!(input.data.is_none());
    }

    #[test]
    fn input_deserializes_with_custom_data() {
        let json = r#"{
            "event_type": "conversion",
            "page_url": "/thank-you",
            "data": {"amount": 199.99, "currency": "USD"}
        }"#;

        let input: CreateAnalyticsEventInput = serde_json::from_str(json).unwrap();

        assert!(input.data.is_some());
        let data = input.data.unwrap();
        assert_eq!(data["amount"], 199.99);
        assert_eq!(data["currency"], "USD");
    }
}

mod create_analytics_event_batch_input_tests {
    use super::*;

    #[test]
    fn batch_stores_events() {
        let events = vec![
            CreateAnalyticsEventInput {
                event_type: AnalyticsEventType::PageView,
                page_url: "/page1".to_string(),
                content_id: None,
                slug: None,
                referrer: None,
                data: None,
            },
            CreateAnalyticsEventInput {
                event_type: AnalyticsEventType::Scroll,
                page_url: "/page1".to_string(),
                content_id: None,
                slug: None,
                referrer: None,
                data: None,
            },
        ];

        let batch = CreateAnalyticsEventBatchInput { events };

        assert_eq!(batch.events.len(), 2);
        assert_eq!(batch.events[0].event_type, AnalyticsEventType::PageView);
        assert_eq!(batch.events[1].event_type, AnalyticsEventType::Scroll);
    }

    #[test]
    fn batch_is_clone() {
        let batch = CreateAnalyticsEventBatchInput {
            events: vec![CreateAnalyticsEventInput {
                event_type: AnalyticsEventType::PageExit,
                page_url: "/exit".to_string(),
                content_id: None,
                slug: None,
                referrer: None,
                data: None,
            }],
        };
        let cloned = batch.clone();

        assert_eq!(batch.events.len(), cloned.events.len());
    }

    #[test]
    fn batch_is_debug() {
        let batch = CreateAnalyticsEventBatchInput { events: vec![] };
        let debug_str = format!("{:?}", batch);
        assert!(debug_str.contains("CreateAnalyticsEventBatchInput"));
    }

    #[test]
    fn batch_deserializes() {
        let json = r#"{
            "events": [
                {"event_type": "page_view", "page_url": "/a"},
                {"event_type": "page_exit", "page_url": "/a"}
            ]
        }"#;

        let batch: CreateAnalyticsEventBatchInput = serde_json::from_str(json).unwrap();

        assert_eq!(batch.events.len(), 2);
    }

    #[test]
    fn batch_deserializes_empty() {
        let json = r#"{"events": []}"#;
        let batch: CreateAnalyticsEventBatchInput = serde_json::from_str(json).unwrap();
        assert!(batch.events.is_empty());
    }
}

mod analytics_event_created_tests {
    use super::*;

    #[test]
    fn created_stores_id_and_type() {
        let created = AnalyticsEventCreated {
            id: "evt_123".to_string(),
            event_type: "page_view".to_string(),
        };

        assert_eq!(created.id, "evt_123");
        assert_eq!(created.event_type, "page_view");
    }

    #[test]
    fn created_serializes() {
        let created = AnalyticsEventCreated {
            id: "evt_456".to_string(),
            event_type: "scroll".to_string(),
        };

        let json = serde_json::to_string(&created).unwrap();
        assert!(json.contains("evt_456"));
        assert!(json.contains("scroll"));
    }

    #[test]
    fn created_is_clone() {
        let created = AnalyticsEventCreated {
            id: "evt_789".to_string(),
            event_type: "engagement".to_string(),
        };
        let cloned = created.clone();

        assert_eq!(created.id, cloned.id);
        assert_eq!(created.event_type, cloned.event_type);
    }

    #[test]
    fn created_is_debug() {
        let created = AnalyticsEventCreated {
            id: "test".to_string(),
            event_type: "test".to_string(),
        };
        let debug_str = format!("{:?}", created);
        assert!(debug_str.contains("AnalyticsEventCreated"));
    }
}

mod analytics_event_batch_response_tests {
    use super::*;

    #[test]
    fn response_stores_recorded_count() {
        let response = AnalyticsEventBatchResponse {
            recorded: 5,
            events: vec![],
        };

        assert_eq!(response.recorded, 5);
    }

    #[test]
    fn response_stores_events() {
        let events = vec![
            AnalyticsEventCreated {
                id: "evt_1".to_string(),
                event_type: "page_view".to_string(),
            },
            AnalyticsEventCreated {
                id: "evt_2".to_string(),
                event_type: "scroll".to_string(),
            },
        ];

        let response = AnalyticsEventBatchResponse {
            recorded: 2,
            events,
        };

        assert_eq!(response.recorded, 2);
        assert_eq!(response.events.len(), 2);
        assert_eq!(response.events[0].id, "evt_1");
        assert_eq!(response.events[1].id, "evt_2");
    }

    #[test]
    fn response_serializes() {
        let response = AnalyticsEventBatchResponse {
            recorded: 1,
            events: vec![AnalyticsEventCreated {
                id: "test".to_string(),
                event_type: "page_view".to_string(),
            }],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("recorded"));
        assert!(json.contains("events"));
        assert!(json.contains("test"));
    }

    #[test]
    fn response_is_clone() {
        let response = AnalyticsEventBatchResponse {
            recorded: 3,
            events: vec![],
        };
        let cloned = response.clone();

        assert_eq!(response.recorded, cloned.recorded);
    }

    #[test]
    fn response_is_debug() {
        let response = AnalyticsEventBatchResponse {
            recorded: 0,
            events: vec![],
        };
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("AnalyticsEventBatchResponse"));
    }
}
