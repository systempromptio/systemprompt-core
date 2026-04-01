//! Tests for analytics event response types.

use systemprompt_analytics::{AnalyticsEventBatchResponse, AnalyticsEventCreated};

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
    fn response_is_debug() {
        let response = AnalyticsEventBatchResponse {
            recorded: 0,
            events: vec![],
        };
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("AnalyticsEventBatchResponse"));
    }
}
