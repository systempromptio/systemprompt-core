//! Tests for inject_utm_params, UtmParams, and edge cases

use systemprompt_content::models::UtmParams;
use systemprompt_content::services::LinkGenerationService;

// ============================================================================
// inject_utm_params Tests
// ============================================================================

#[test]
fn test_inject_utm_params_empty_params() {
    let params = UtmParams {
        source: None,
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com/page";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert_eq!(result, url);
}

#[test]
fn test_inject_utm_params_single_param() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com/page";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert_eq!(result, "https://example.com/page?utm_source=google");
}

#[test]
fn test_inject_utm_params_multiple_params() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: Some("cpc".to_string()),
        campaign: Some("summer".to_string()),
        term: None,
        content: None,
    };

    let url = "https://example.com/page";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=google"));
    assert!(result.contains("utm_medium=cpc"));
    assert!(result.contains("utm_campaign=summer"));
    assert!(result.starts_with("https://example.com/page?"));
}

#[test]
fn test_inject_utm_params_all_params() {
    let params = UtmParams {
        source: Some("source".to_string()),
        medium: Some("medium".to_string()),
        campaign: Some("campaign".to_string()),
        term: Some("term".to_string()),
        content: Some("content".to_string()),
    };

    let url = "https://example.com";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=source"));
    assert!(result.contains("utm_medium=medium"));
    assert!(result.contains("utm_campaign=campaign"));
    assert!(result.contains("utm_term=term"));
    assert!(result.contains("utm_content=content"));
}

#[test]
fn test_inject_utm_params_url_with_existing_query() {
    let params = UtmParams {
        source: Some("twitter".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com/page?existing=param";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert_eq!(
        result,
        "https://example.com/page?existing=param&utm_source=twitter"
    );
}

#[test]
fn test_inject_utm_params_url_with_fragment() {
    let params = UtmParams {
        source: Some("email".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com/page#section";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=email"));
}

// ============================================================================
// Edge Cases for inject_utm_params
// ============================================================================

#[test]
fn test_inject_utm_params_special_characters() {
    let params = UtmParams {
        source: Some("email+newsletter".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=email%2Bnewsletter") || result.contains("utm_source=email+newsletter"));
}

#[test]
fn test_inject_utm_params_empty_url() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=google"));
}

#[test]
fn test_inject_utm_params_url_only_query_mark() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com?";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("&utm_source=google"));
}

// ============================================================================
// UtmParams Tests
// ============================================================================

#[test]
fn test_utm_params_to_query_string_empty() {
    let params = UtmParams {
        source: None,
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let query = params.to_query_string();
    assert!(query.is_empty());
}

#[test]
fn test_utm_params_to_query_string_single() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let query = params.to_query_string();
    assert_eq!(query, "utm_source=google");
}

#[test]
fn test_utm_params_to_query_string_multiple() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: Some("cpc".to_string()),
        campaign: None,
        term: None,
        content: None,
    };

    let query = params.to_query_string();
    assert!(query.contains("utm_source=google"));
    assert!(query.contains("utm_medium=cpc"));
    assert!(query.contains("&"));
}

#[test]
fn test_utm_params_to_json() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: Some("cpc".to_string()),
        campaign: Some("summer".to_string()),
        term: None,
        content: None,
    };

    let json = params.to_json().unwrap();
    assert!(json.contains("\"source\":\"google\""));
    assert!(json.contains("\"medium\":\"cpc\""));
    assert!(json.contains("\"campaign\":\"summer\""));
}

#[test]
fn test_utm_params_deserialize() {
    let json = r#"{"source":"twitter","medium":"social","campaign":"winter"}"#;
    let params: UtmParams = serde_json::from_str(json).unwrap();

    assert_eq!(params.source, Some("twitter".to_string()));
    assert_eq!(params.medium, Some("social".to_string()));
    assert_eq!(params.campaign, Some("winter".to_string()));
    assert!(params.term.is_none());
    assert!(params.content.is_none());
}

#[test]
fn test_utm_params_deserialize_empty() {
    let json = r#"{}"#;
    let params: UtmParams = serde_json::from_str(json).unwrap();

    assert!(params.source.is_none());
    assert!(params.medium.is_none());
    assert!(params.campaign.is_none());
    assert!(params.term.is_none());
    assert!(params.content.is_none());
}

#[test]
fn test_utm_params_deserialize_invalid() {
    let json = "not valid json";
    let result: Result<UtmParams, _> = serde_json::from_str(json);
    result.unwrap_err();
}
