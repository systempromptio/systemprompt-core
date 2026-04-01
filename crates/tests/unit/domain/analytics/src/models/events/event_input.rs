//! Tests for analytics event input types.

use systemprompt_analytics::{
    AnalyticsEventType, CreateAnalyticsEventBatchInput, CreateAnalyticsEventInput,
};

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
