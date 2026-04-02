use systemprompt_models::{
    AcceptedResponse, ApiError, ApiResponse, CollectionResponse, CreatedResponse, ErrorCode,
    PaginationInfo, PaginationParams, SearchQuery, SingleResponse, SortOrder, SortParams,
    SuccessResponse, ValidationError,
};

use systemprompt_models::api::responses::{
    DiscoveryResponse, Link, MarkdownFrontmatter, MarkdownResponse, ResponseLinks, ResponseMeta,
};

#[test]
fn pagination_info_single_page() {
    let info = PaginationInfo::new(5, 1, 10);
    assert_eq!(info.total, 5);
    assert_eq!(info.page, 1);
    assert_eq!(info.per_page, 10);
    assert_eq!(info.total_pages, 1);
    assert!(!info.has_next);
    assert!(!info.has_prev);
    assert!(info.next_url.is_none());
    assert!(info.prev_url.is_none());
}

#[test]
fn pagination_info_multiple_pages() {
    let info = PaginationInfo::new(25, 2, 10);
    assert_eq!(info.total_pages, 3);
    assert!(info.has_next);
    assert!(info.has_prev);
}

#[test]
fn pagination_info_first_page() {
    let info = PaginationInfo::new(30, 1, 10);
    assert!(info.has_next);
    assert!(!info.has_prev);
}

#[test]
fn pagination_info_last_page() {
    let info = PaginationInfo::new(30, 3, 10);
    assert!(!info.has_next);
    assert!(info.has_prev);
}

#[test]
fn pagination_info_zero_items() {
    let info = PaginationInfo::new(0, 1, 10);
    assert_eq!(info.total, 0);
    assert_eq!(info.total_pages, 0);
    assert!(!info.has_next);
    assert!(!info.has_prev);
}

#[test]
fn pagination_info_exact_page_boundary() {
    let info = PaginationInfo::new(20, 1, 10);
    assert_eq!(info.total_pages, 2);
    assert!(info.has_next);
}

#[test]
fn pagination_info_exact_single_page_boundary() {
    let info = PaginationInfo::new(10, 1, 10);
    assert_eq!(info.total_pages, 1);
    assert!(!info.has_next);
}

#[test]
fn pagination_info_with_base_url_generates_next() {
    let info = PaginationInfo::new(30, 1, 10).with_base_url("/api/items");
    assert_eq!(
        info.next_url.as_deref(),
        Some("/api/items?page=2&per_page=10")
    );
    assert!(info.prev_url.is_none());
}

#[test]
fn pagination_info_with_base_url_generates_prev() {
    let info = PaginationInfo::new(30, 3, 10).with_base_url("/api/items");
    assert!(info.next_url.is_none());
    assert_eq!(
        info.prev_url.as_deref(),
        Some("/api/items?page=2&per_page=10")
    );
}

#[test]
fn pagination_info_with_base_url_generates_both() {
    let info = PaginationInfo::new(30, 2, 10).with_base_url("/api/items");
    assert!(info.next_url.is_some());
    assert!(info.prev_url.is_some());
}

#[test]
fn pagination_info_serde_roundtrip() {
    let info = PaginationInfo::new(100, 3, 20);
    let json = serde_json::to_string(&info).unwrap();
    let deserialized: PaginationInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.total, 100);
    assert_eq!(deserialized.page, 3);
    assert_eq!(deserialized.per_page, 20);
    assert_eq!(deserialized.total_pages, 5);
}

#[test]
fn pagination_params_default_values() {
    let params = PaginationParams::default();
    assert_eq!(params.page, 1);
    assert_eq!(params.per_page, 20);
}

#[test]
fn pagination_params_offset_calculation() {
    let params = PaginationParams {
        page: 3,
        per_page: 10,
    };
    assert_eq!(params.offset(), 20);
    assert_eq!(params.limit(), 10);
}

#[test]
fn pagination_params_first_page_offset() {
    let params = PaginationParams {
        page: 1,
        per_page: 25,
    };
    assert_eq!(params.offset(), 0);
}

#[test]
fn api_response_wraps_data() {
    let response = ApiResponse::new("test_data");
    assert_eq!(response.data, "test_data");
    assert!(response.links.is_none());
    assert_eq!(response.meta.version, "1.0.0");
}

#[test]
fn api_response_with_links() {
    let links = ResponseLinks {
        self_link: "/api/test".to_string(),
        next: Some("/api/test?page=2".to_string()),
        prev: None,
        docs: "/docs".to_string(),
    };
    let response = ApiResponse::new(42).with_links(links);
    assert!(response.links.is_some());
    assert_eq!(response.links.unwrap().self_link, "/api/test");
}

#[test]
fn api_response_with_meta() {
    let meta = ResponseMeta::new();
    let response = ApiResponse::new("data").with_meta(meta);
    assert_eq!(response.meta.version, "1.0.0");
    assert!(response.meta.pagination.is_none());
}

#[test]
fn api_response_serde_roundtrip() {
    let response = ApiResponse::new(vec![1, 2, 3]);
    let json = serde_json::to_string(&response).unwrap();
    let deserialized: ApiResponse<Vec<i32>> = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.data, vec![1, 2, 3]);
}

#[test]
fn single_response_wraps_data() {
    let response = SingleResponse::new("single_item");
    assert_eq!(response.data, "single_item");
    assert!(response.links.is_none());
}

#[test]
fn single_response_with_links() {
    let links = ResponseLinks {
        self_link: "/api/item/1".to_string(),
        next: None,
        prev: None,
        docs: "/docs".to_string(),
    };
    let response = SingleResponse::new(42).with_links(links);
    assert!(response.links.is_some());
}

#[test]
fn collection_response_wraps_vec() {
    let response = CollectionResponse::new(vec![1, 2, 3]);
    assert_eq!(response.data.len(), 3);
    assert!(response.meta.pagination.is_none());
}

#[test]
fn collection_response_paginated() {
    let pagination = PaginationInfo::new(50, 1, 10);
    let response = CollectionResponse::paginated(vec![1, 2, 3], pagination);
    assert_eq!(response.data.len(), 3);
    let page_info = response.meta.pagination.unwrap();
    assert_eq!(page_info.total, 50);
    assert_eq!(page_info.total_pages, 5);
}

#[test]
fn collection_response_paginated_empty() {
    let pagination = PaginationInfo::new(0, 1, 10);
    let response: CollectionResponse<String> = CollectionResponse::paginated(vec![], pagination);
    assert!(response.data.is_empty());
    assert_eq!(response.meta.pagination.unwrap().total, 0);
}

#[test]
fn success_response_message() {
    let response = SuccessResponse::new("Operation completed");
    assert_eq!(response.message, "Operation completed");
}

#[test]
fn created_response_with_location() {
    let response = CreatedResponse::new(42, "/api/items/42");
    assert_eq!(response.data, 42);
    assert_eq!(response.location, "/api/items/42");
}

#[test]
fn accepted_response_basic() {
    let response = AcceptedResponse::new("Processing started");
    assert_eq!(response.message, "Processing started");
    assert!(response.job_id.is_none());
    assert!(response.status_url.is_none());
}

#[test]
fn accepted_response_with_job() {
    let response =
        AcceptedResponse::new("Processing").with_job("job-123", "/api/jobs/job-123/status");
    assert_eq!(response.job_id.as_deref(), Some("job-123"));
    assert_eq!(
        response.status_url.as_deref(),
        Some("/api/jobs/job-123/status")
    );
}

#[test]
fn response_meta_default() {
    let meta = ResponseMeta::default();
    assert_eq!(meta.version, "1.0.0");
    assert!(meta.pagination.is_none());
}

#[test]
fn response_meta_with_pagination() {
    let pagination = PaginationInfo::new(100, 1, 10);
    let meta = ResponseMeta::new().with_pagination(pagination);
    assert!(meta.pagination.is_some());
    assert_eq!(meta.pagination.unwrap().total, 100);
}

#[test]
fn api_error_not_found() {
    let error = ApiError::not_found("Resource not found");
    assert!(matches!(error.code, ErrorCode::NotFound));
    assert_eq!(error.message, "Resource not found");
    assert!(error.details.is_none());
}

#[test]
fn api_error_bad_request() {
    let error = ApiError::bad_request("Invalid input");
    assert!(matches!(error.code, ErrorCode::BadRequest));
    assert_eq!(error.message, "Invalid input");
}

#[test]
fn api_error_unauthorized() {
    let error = ApiError::unauthorized("Token expired");
    assert!(matches!(error.code, ErrorCode::Unauthorized));
    assert_eq!(error.message, "Token expired");
}

#[test]
fn api_error_forbidden() {
    let error = ApiError::forbidden("Access denied");
    assert!(matches!(error.code, ErrorCode::Forbidden));
}

#[test]
fn api_error_internal_error() {
    let error = ApiError::internal_error("Something went wrong");
    assert!(matches!(error.code, ErrorCode::InternalError));
}

#[test]
fn api_error_conflict() {
    let error = ApiError::conflict("Resource already exists");
    assert!(matches!(error.code, ErrorCode::ConflictError));
}

#[test]
fn api_error_with_details() {
    let error = ApiError::not_found("Not found").with_details("The item was deleted");
    assert_eq!(error.details.as_deref(), Some("The item was deleted"));
}

#[test]
fn api_error_with_error_key() {
    let error = ApiError::bad_request("Bad").with_error_key("INVALID_FORMAT");
    assert_eq!(error.error_key.as_deref(), Some("INVALID_FORMAT"));
}

#[test]
fn api_error_with_path() {
    let error = ApiError::not_found("Missing").with_path("/api/users/123");
    assert_eq!(error.path.as_deref(), Some("/api/users/123"));
}

#[test]
fn api_error_with_trace_id() {
    let error = ApiError::internal_error("Oops").with_trace_id("trace-abc-123");
    assert_eq!(error.trace_id.as_deref(), Some("trace-abc-123"));
}

#[test]
fn api_error_validation_error_with_errors() {
    let validation_errors = vec![ValidationError {
        field: "email".to_string(),
        message: "Invalid email format".to_string(),
        code: "invalid_format".to_string(),
        context: None,
    }];
    let error = ApiError::validation_error("Validation failed", validation_errors);
    assert!(matches!(error.code, ErrorCode::ValidationError));
    assert_eq!(error.validation_errors.len(), 1);
    assert_eq!(error.validation_errors[0].field, "email");
}

#[test]
fn api_error_serde_roundtrip() {
    let error = ApiError::not_found("Item not found")
        .with_details("Checked all stores")
        .with_path("/api/items/999");
    let json = serde_json::to_string(&error).unwrap();
    let deserialized: ApiError = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized.code, ErrorCode::NotFound));
    assert_eq!(deserialized.message, "Item not found");
    assert_eq!(deserialized.details.as_deref(), Some("Checked all stores"));
}

#[test]
fn error_code_serde_roundtrip() {
    let codes = vec![
        ErrorCode::NotFound,
        ErrorCode::BadRequest,
        ErrorCode::Unauthorized,
        ErrorCode::Forbidden,
        ErrorCode::InternalError,
        ErrorCode::ValidationError,
        ErrorCode::ConflictError,
        ErrorCode::RateLimited,
        ErrorCode::ServiceUnavailable,
    ];
    for code in codes {
        let json = serde_json::to_string(&code).unwrap();
        let deserialized: ErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(
            std::mem::discriminant(&code),
            std::mem::discriminant(&deserialized)
        );
    }
}

#[test]
fn sort_order_default_is_asc() {
    let default = SortOrder::default();
    assert!(matches!(default, SortOrder::Asc));
}

#[test]
fn sort_order_serde_roundtrip() {
    let asc_json = serde_json::to_string(&SortOrder::Asc).unwrap();
    let desc_json = serde_json::to_string(&SortOrder::Desc).unwrap();
    assert_eq!(asc_json, "\"asc\"");
    assert_eq!(desc_json, "\"desc\"");
}

#[test]
fn link_new_with_title() {
    let link = Link::new("/api/items", Some("Items".to_string()));
    assert_eq!(link.href, "/api/items");
    assert_eq!(link.title.as_deref(), Some("Items"));
}

#[test]
fn link_new_without_title() {
    let link = Link::new("/api/items", None);
    assert_eq!(link.href, "/api/items");
    assert!(link.title.is_none());
}

#[test]
fn discovery_response_with_links() {
    let mut links = indexmap::IndexMap::new();
    links.insert("self".to_string(), Link::new("/api", None));
    links.insert(
        "users".to_string(),
        Link::new("/api/users", Some("Users".to_string())),
    );
    let response = DiscoveryResponse::new("API v1", links);
    assert_eq!(response.data, "API v1");
    assert_eq!(response.links.len(), 2);
}

#[test]
fn markdown_frontmatter_new() {
    let fm = MarkdownFrontmatter::new("Test Title", "test-slug");
    assert_eq!(fm.title, "Test Title");
    assert_eq!(fm.slug, "test-slug");
    assert!(fm.description.is_none());
    assert!(fm.tags.is_empty());
}

#[test]
fn markdown_frontmatter_builder_chain() {
    let fm = MarkdownFrontmatter::new("Title", "slug")
        .with_description("A description")
        .with_author("Author Name")
        .with_tags(vec!["rust".to_string(), "test".to_string()])
        .with_url("https://example.com/slug");
    assert_eq!(fm.description.as_deref(), Some("A description"));
    assert_eq!(fm.author.as_deref(), Some("Author Name"));
    assert_eq!(fm.tags.len(), 2);
    assert_eq!(fm.url.as_deref(), Some("https://example.com/slug"));
}

#[test]
fn markdown_response_to_markdown_format() {
    let fm = MarkdownFrontmatter::new("Title", "slug");
    let response = MarkdownResponse::new(fm, "Body content here");
    let md = response.to_markdown();
    assert!(md.starts_with("---\n"));
    assert!(md.contains("Body content here"));
}

#[test]
fn search_query_serde() {
    let query = SearchQuery {
        search: Some("test".to_string()),
        sort_by: Some("name".to_string()),
        sort_dir: Some("asc".to_string()),
    };
    let json = serde_json::to_string(&query).unwrap();
    let deserialized: SearchQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.search.as_deref(), Some("test"));
}

#[test]
fn sort_params_serde() {
    let params = SortParams {
        sort_by: Some("created_at".to_string()),
        sort_order: SortOrder::Desc,
    };
    let json = serde_json::to_string(&params).unwrap();
    let deserialized: SortParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.sort_by.as_deref(), Some("created_at"));
}

#[test]
fn validation_error_serde_roundtrip() {
    let error = ValidationError {
        field: "username".to_string(),
        message: "Too short".to_string(),
        code: "min_length".to_string(),
        context: Some(serde_json::json!({"min": 3})),
    };
    let json = serde_json::to_string(&error).unwrap();
    let deserialized: ValidationError = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.field, "username");
    assert!(deserialized.context.is_some());
}
