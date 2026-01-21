//! Unit tests for search models
//!
//! Tests cover:
//! - SearchRequest struct
//! - SearchFilters struct
//! - SearchResult struct
//! - SearchResponse struct

use systemprompt_content::{SearchFilters, SearchRequest, SearchResponse, SearchResult};
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

// ============================================================================
// SearchRequest Tests
// ============================================================================

#[test]
fn test_search_request_minimal() {
    let json = r#"{"query": "rust programming"}"#;
    let request: SearchRequest = serde_json::from_str(json).unwrap();

    assert_eq!(request.query, "rust programming");
    assert!(request.filters.is_none());
    assert!(request.limit.is_none());
}

#[test]
fn test_search_request_with_limit() {
    let json = r#"{"query": "test", "limit": 20}"#;
    let request: SearchRequest = serde_json::from_str(json).unwrap();

    assert_eq!(request.query, "test");
    assert_eq!(request.limit, Some(20));
}

#[test]
fn test_search_request_with_filters() {
    let json = r#"{
        "query": "example",
        "filters": {
            "category_id": "tech"
        }
    }"#;
    let request: SearchRequest = serde_json::from_str(json).unwrap();

    assert_eq!(request.query, "example");
    assert!(request.filters.is_some());
    let filters = request.filters.unwrap();
    assert_eq!(filters.category_id.unwrap().as_str(), "tech");
}

#[test]
fn test_search_request_full() {
    let json = r#"{
        "query": "complete query",
        "filters": {
            "category_id": "programming"
        },
        "limit": 50
    }"#;
    let request: SearchRequest = serde_json::from_str(json).unwrap();

    assert_eq!(request.query, "complete query");
    assert_eq!(request.limit, Some(50));
    assert!(request.filters.is_some());
}

#[test]
fn test_search_request_serialization() {
    let request = SearchRequest {
        query: "serialize test".to_string(),
        filters: Some(SearchFilters {
            category_id: Some(CategoryId::new("cat1")),
        }),
        limit: Some(10),
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"query\":\"serialize test\""));
    assert!(json.contains("\"limit\":10"));
}

#[test]
fn test_search_request_clone() {
    let request = SearchRequest {
        query: "clone test".to_string(),
        filters: None,
        limit: Some(5),
    };

    let cloned = request.clone();
    assert_eq!(cloned.query, request.query);
    assert_eq!(cloned.limit, request.limit);
}

// ============================================================================
// SearchFilters Tests
// ============================================================================

#[test]
fn test_search_filters_with_category() {
    let filters = SearchFilters {
        category_id: Some(CategoryId::new("articles")),
    };

    assert_eq!(filters.category_id.as_ref().unwrap().as_str(), "articles");
}

#[test]
fn test_search_filters_empty() {
    let json = r#"{"category_id": null}"#;
    let filters: SearchFilters = serde_json::from_str(json).unwrap();

    assert!(filters.category_id.is_none());
}

#[test]
fn test_search_filters_serialization() {
    let filters = SearchFilters {
        category_id: Some(CategoryId::new("test-category")),
    };

    let json = serde_json::to_string(&filters).unwrap();
    assert!(json.contains("test-category"));
}

#[test]
fn test_search_filters_clone() {
    let filters = SearchFilters {
        category_id: Some(CategoryId::new("cloneable")),
    };

    let cloned = filters.clone();
    assert_eq!(
        cloned.category_id.as_ref().unwrap().as_str(),
        filters.category_id.as_ref().unwrap().as_str()
    );
}

// ============================================================================
// SearchResult Tests
// ============================================================================

#[test]
fn test_search_result_creation() {
    let result = SearchResult {
        id: ContentId::new("result-1"),
        slug: "test-result".to_string(),
        title: "Test Result".to_string(),
        description: "A test search result".to_string(),
        image: Some("/images/test.png".to_string()),
        view_count: 100,
        source_id: SourceId::new("blog"),
        category_id: Some(CategoryId::new("tech")),
    };

    assert_eq!(result.id.as_str(), "result-1");
    assert_eq!(result.slug, "test-result");
    assert_eq!(result.title, "Test Result");
    assert_eq!(result.description, "A test search result");
    assert_eq!(result.image, Some("/images/test.png".to_string()));
    assert_eq!(result.view_count, 100);
    assert_eq!(result.source_id.as_str(), "blog");
    assert_eq!(result.category_id.as_ref().unwrap().as_str(), "tech");
}

#[test]
fn test_search_result_without_image() {
    let result = SearchResult {
        id: ContentId::new("no-img"),
        slug: "no-image".to_string(),
        title: "No Image".to_string(),
        description: "Content without image".to_string(),
        image: None,
        view_count: 0,
        source_id: SourceId::new("docs"),
        category_id: None,
    };

    assert!(result.image.is_none());
    assert!(result.category_id.is_none());
}

#[test]
fn test_search_result_serialization() {
    let result = SearchResult {
        id: ContentId::new("ser-1"),
        slug: "serialized".to_string(),
        title: "Serialized".to_string(),
        description: "Desc".to_string(),
        image: None,
        view_count: 50,
        source_id: SourceId::new("src"),
        category_id: None,
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"slug\":\"serialized\""));
    assert!(json.contains("\"view_count\":50"));
}

#[test]
fn test_search_result_clone() {
    let result = SearchResult {
        id: ContentId::new("clone-id"),
        slug: "clone".to_string(),
        title: "Clone".to_string(),
        description: "Clone desc".to_string(),
        image: Some("/img.png".to_string()),
        view_count: 10,
        source_id: SourceId::new("source"),
        category_id: Some(CategoryId::new("cat")),
    };

    let cloned = result.clone();
    assert_eq!(cloned.id.as_str(), result.id.as_str());
    assert_eq!(cloned.slug, result.slug);
    assert_eq!(cloned.view_count, result.view_count);
}

// ============================================================================
// SearchResponse Tests
// ============================================================================

#[test]
fn test_search_response_empty() {
    let response = SearchResponse {
        results: vec![],
        total: 0,
    };

    assert!(response.results.is_empty());
    assert_eq!(response.total, 0);
}

#[test]
fn test_search_response_with_results() {
    let results = vec![
        SearchResult {
            id: ContentId::new("r1"),
            slug: "result-1".to_string(),
            title: "Result 1".to_string(),
            description: "First result".to_string(),
            image: None,
            view_count: 10,
            source_id: SourceId::new("blog"),
            category_id: None,
        },
        SearchResult {
            id: ContentId::new("r2"),
            slug: "result-2".to_string(),
            title: "Result 2".to_string(),
            description: "Second result".to_string(),
            image: Some("/img2.png".to_string()),
            view_count: 20,
            source_id: SourceId::new("docs"),
            category_id: Some(CategoryId::new("tech")),
        },
    ];

    let response = SearchResponse {
        results,
        total: 2,
    };

    assert_eq!(response.results.len(), 2);
    assert_eq!(response.total, 2);
    assert_eq!(response.results[0].slug, "result-1");
    assert_eq!(response.results[1].slug, "result-2");
}

#[test]
fn test_search_response_serialization() {
    let response = SearchResponse {
        results: vec![SearchResult {
            id: ContentId::new("s1"),
            slug: "slug".to_string(),
            title: "Title".to_string(),
            description: "Desc".to_string(),
            image: None,
            view_count: 5,
            source_id: SourceId::new("src"),
            category_id: None,
        }],
        total: 1,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"total\":1"));
    assert!(json.contains("\"results\":["));
}

#[test]
fn test_search_response_clone() {
    let response = SearchResponse {
        results: vec![],
        total: 0,
    };

    let cloned = response.clone();
    assert_eq!(cloned.total, response.total);
    assert_eq!(cloned.results.len(), response.results.len());
}
