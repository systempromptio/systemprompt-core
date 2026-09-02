use systemprompt_models::routing::{
    ApiCategory, AssetType, EventMetadata, RouteClassifier, RouteType,
};

fn classifier_without_content_routing() -> RouteClassifier {
    RouteClassifier::new(None)
}

mod event_metadata_tests {
    use super::*;

    #[test]
    fn html_content_metadata_fields() {
        let meta = EventMetadata::HTML_CONTENT;
        assert_eq!(meta.event_type, "page_view");
        assert_eq!(meta.event_category, "content");
        assert_eq!(meta.log_module, "page_view");
    }

    #[test]
    fn api_request_metadata_fields() {
        let meta = EventMetadata::API_REQUEST;
        assert_eq!(meta.event_type, "http_request");
        assert_eq!(meta.event_category, "api");
        assert_eq!(meta.log_module, "http_request");
    }

    #[test]
    fn static_asset_metadata_fields() {
        let meta = EventMetadata::STATIC_ASSET;
        assert_eq!(meta.event_type, "asset_request");
        assert_eq!(meta.event_category, "static");
        assert_eq!(meta.log_module, "asset_request");
    }

    #[test]
    fn not_found_metadata_fields() {
        let meta = EventMetadata::NOT_FOUND;
        assert_eq!(meta.event_type, "not_found");
        assert_eq!(meta.event_category, "error");
        assert_eq!(meta.log_module, "not_found");
    }

    #[test]
    fn metadata_eq_same_variant() {
        assert_eq!(EventMetadata::HTML_CONTENT, EventMetadata::HTML_CONTENT);
    }

    #[test]
    fn metadata_ne_different_variant() {
        assert_ne!(EventMetadata::HTML_CONTENT, EventMetadata::API_REQUEST);
    }
}

mod route_classifier_classify_tests {
    use super::*;

    #[test]
    fn classifies_js_file_as_static_asset() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/app.js", "GET");
        assert!(matches!(
            route,
            RouteType::StaticAsset {
                asset_type: AssetType::JavaScript
            }
        ));
    }

    #[test]
    fn classifies_css_file_as_stylesheet() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/styles.css", "GET");
        assert!(matches!(
            route,
            RouteType::StaticAsset {
                asset_type: AssetType::Stylesheet
            }
        ));
    }

    #[test]
    fn classifies_png_as_image() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/logo.png", "GET");
        assert!(matches!(
            route,
            RouteType::StaticAsset {
                asset_type: AssetType::Image
            }
        ));
    }

    #[test]
    fn classifies_woff2_as_font() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/font.woff2", "GET");
        assert!(matches!(
            route,
            RouteType::StaticAsset {
                asset_type: AssetType::Font
            }
        ));
    }

    #[test]
    fn classifies_map_as_source_map() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/app.js.map", "GET");
        assert!(matches!(
            route,
            RouteType::StaticAsset {
                asset_type: AssetType::SourceMap
            }
        ));
    }

    #[test]
    fn classifies_assets_path_as_static() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/assets/something", "GET");
        assert!(matches!(route, RouteType::StaticAsset { .. }));
    }

    #[test]
    fn classifies_wellknown_path_as_static() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/.well-known/something", "GET");
        assert!(matches!(route, RouteType::StaticAsset { .. }));
    }

    #[test]
    fn classifies_api_content_path() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/api/v1/content/pages", "GET");
        assert!(matches!(
            route,
            RouteType::ApiEndpoint {
                category: ApiCategory::Content
            }
        ));
    }

    #[test]
    fn classifies_api_core_path() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/api/v1/core/contexts", "GET");
        assert!(matches!(
            route,
            RouteType::ApiEndpoint {
                category: ApiCategory::Core
            }
        ));
    }

    #[test]
    fn classifies_api_agents_path() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/api/v1/agents/registry", "GET");
        assert!(matches!(
            route,
            RouteType::ApiEndpoint {
                category: ApiCategory::Agents
            }
        ));
    }

    #[test]
    fn classifies_api_oauth_path_as_core() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/api/v1/core/oauth/token", "POST");
        assert!(matches!(
            route,
            RouteType::ApiEndpoint {
                category: ApiCategory::Core
            }
        ));
    }

    #[test]
    fn classifies_api_other_path() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/api/v1/health", "GET");
        assert!(matches!(
            route,
            RouteType::ApiEndpoint {
                category: ApiCategory::Other
            }
        ));
    }

    #[test]
    fn classifies_track_path_as_api_other() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/track/engagement", "POST");
        assert!(matches!(
            route,
            RouteType::ApiEndpoint {
                category: ApiCategory::Other
            }
        ));
    }

    #[test]
    fn classifies_non_api_non_static_as_html_without_content_routing() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/about", "GET");
        assert!(matches!(route, RouteType::HtmlContent { source } if source == "unknown"));
    }

    #[test]
    fn favicon_classified_as_static() {
        let classifier = classifier_without_content_routing();
        let route = classifier.classify("/favicon.ico", "GET");
        assert!(matches!(route, RouteType::StaticAsset { .. }));
    }
}

mod route_classifier_analytics_tests {
    use super::*;

    #[test]
    fn options_requests_not_tracked() {
        let classifier = classifier_without_content_routing();
        assert!(!classifier.should_track_analytics("/api/v1/core/contexts", "OPTIONS"));
    }

    #[test]
    fn html_content_tracked() {
        let classifier = classifier_without_content_routing();
        assert!(classifier.should_track_analytics("/about", "GET"));
    }

    #[test]
    fn static_assets_not_tracked() {
        let classifier = classifier_without_content_routing();
        assert!(!classifier.should_track_analytics("/app.js", "GET"));
    }

    #[test]
    fn api_core_tracked() {
        let classifier = classifier_without_content_routing();
        assert!(classifier.should_track_analytics("/api/v1/core/contexts", "GET"));
    }

    #[test]
    fn api_agents_not_tracked() {
        let classifier = classifier_without_content_routing();
        assert!(!classifier.should_track_analytics("/api/v1/agents/registry", "GET"));
    }
}

mod route_classifier_helpers_tests {
    use super::*;

    #[test]
    fn is_html_true_for_page_path() {
        let classifier = classifier_without_content_routing();
        assert!(classifier.is_html("/about"));
    }

    #[test]
    fn is_html_false_for_js_file() {
        let classifier = classifier_without_content_routing();
        assert!(!classifier.is_html("/app.js"));
    }

    #[test]
    fn is_html_false_for_api_path() {
        let classifier = classifier_without_content_routing();
        assert!(!classifier.is_html("/api/v1/health"));
    }

    #[test]
    fn get_event_metadata_for_html() {
        let classifier = classifier_without_content_routing();
        let meta = classifier.get_event_metadata("/about", "GET");
        assert_eq!(meta, EventMetadata::HTML_CONTENT);
    }

    #[test]
    fn get_event_metadata_for_api() {
        let classifier = classifier_without_content_routing();
        let meta = classifier.get_event_metadata("/api/v1/core/contexts", "POST");
        assert_eq!(meta, EventMetadata::API_REQUEST);
    }

    #[test]
    fn get_event_metadata_for_static() {
        let classifier = classifier_without_content_routing();
        let meta = classifier.get_event_metadata("/styles.css", "GET");
        assert_eq!(meta, EventMetadata::STATIC_ASSET);
    }

    #[test]
    fn route_classifier_debug() {
        let classifier = classifier_without_content_routing();
        let debug_str = format!("{:?}", classifier);
        assert!(debug_str.contains("RouteClassifier"));
        assert!(debug_str.contains("false"));
    }
}

// The inference gateway is mounted at `/v1`, not under `/api`, so it matched no
// arm of `classify` and fell all the way through to the HTML-content branch.
// Every gateway failure was therefore logged as an HTML page view:
//
//   ERROR analytics::events: module="page_view" HTTP 529 - POST /v1/messages
//
// which is how a run of upstream 529s reads as a website problem.
mod gateway_route_tests {
    use super::*;

    #[test]
    fn gateway_metadata_fields() {
        let meta = EventMetadata::GATEWAY_REQUEST;
        assert_eq!(meta.event_type, "gateway_request");
        assert_eq!(meta.event_category, "gateway");
        assert_eq!(meta.log_module, "gateway_request");
    }

    #[test]
    fn gateway_paths_classify_as_gateway_api_endpoints() {
        let c = classifier_without_content_routing();
        for path in [
            "/v1/messages",
            "/v1/messages?beta=true",
            "/v1/chat/completions",
            "/api/public/gateway/models",
        ] {
            assert_eq!(
                c.classify(path, "POST"),
                RouteType::ApiEndpoint {
                    category: ApiCategory::Gateway
                },
                "{path} must classify as a gateway endpoint"
            );
        }
    }

    #[test]
    fn gateway_requests_are_never_logged_as_page_views() {
        // The regression, stated directly.
        let c = classifier_without_content_routing();
        let meta = c.get_event_metadata("/v1/messages", "POST");
        assert_eq!(meta.log_module, "gateway_request");
        assert_ne!(meta.log_module, "page_view");
    }

    #[test]
    fn gateway_requests_are_not_double_counted_as_web_analytics() {
        // Every inference call already lands a row in `ai_requests` carrying
        // identity, model, tokens and cost. Counting it again here would
        // inflate the web-analytics totals with traffic that has a better
        // record of itself elsewhere.
        let c = classifier_without_content_routing();
        assert!(!c.should_track_analytics("/v1/messages", "POST"));
    }

    #[test]
    fn gateway_paths_are_not_html() {
        let c = classifier_without_content_routing();
        assert!(!c.is_html("/v1/messages"));
    }
}
