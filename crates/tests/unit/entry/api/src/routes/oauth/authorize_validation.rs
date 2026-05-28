use systemprompt_api::routes::oauth::endpoints::authorize::AuthorizeQuery;
use systemprompt_api::routes::oauth::endpoints::authorize::validation::{
    SelfOrigins, validate_oauth_parameters,
};
use systemprompt_identifiers::ClientId;
use url::Url;

fn default_origin() -> SelfOrigins {
    let origin = Url::parse("https://gateway.example.com")
        .expect("test origin parses")
        .origin();
    SelfOrigins::new(origin.clone(), origin)
}

fn origin_of(url: &str) -> SelfOrigins {
    let origin = Url::parse(url).expect("test origin parses").origin();
    SelfOrigins::new(origin.clone(), origin)
}

fn valid_query() -> AuthorizeQuery {
    AuthorizeQuery {
        response_type: "code".to_string(),
        client_id: ClientId::new("sp_test_client"),
        redirect_uri: Some("https://example.com/callback".to_string()),
        scope: Some("openid".to_string()),
        state: Some("random_state_value".to_string()),
        code_challenge: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string()),
        code_challenge_method: Some("S256".to_string()),
        response_mode: None,
        display: None,
        prompt: None,
        max_age: None,
        ui_locales: None,
        resource: None,
    }
}

#[test]
fn test_valid_query_passes_validation() {
    let query = valid_query();
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_response_type_token_rejected() {
    let query = AuthorizeQuery {
        response_type: "token".to_string(),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("response_type"));
}

#[test]
fn test_response_type_empty_rejected() {
    let query = AuthorizeQuery {
        response_type: "".to_string(),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_err());
}

#[test]
fn test_response_mode_query_accepted() {
    let query = AuthorizeQuery {
        response_mode: Some("query".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_response_mode_fragment_rejected() {
    let query = AuthorizeQuery {
        response_mode: Some("fragment".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("response_mode"));
}

#[test]
fn test_response_mode_none_accepted() {
    let query = AuthorizeQuery {
        response_mode: None,
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_code_challenge_missing_rejected() {
    let query = AuthorizeQuery {
        code_challenge: None,
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("code_challenge is required"));
}

#[test]
fn test_code_challenge_too_short_rejected() {
    let query = AuthorizeQuery {
        code_challenge: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEj".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("too short"));
}

#[test]
fn test_code_challenge_exactly_43_chars_accepted() {
    let query = AuthorizeQuery {
        code_challenge: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_code_challenge_128_chars_accepted() {
    let part_a = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let part_b = "QmN8vTx3YpRaLs6Uc0IwHbDfKgEoAi7Zn5Vy2XrJlM";
    let part_c = "q9Wt4SuGhPkCe1Od3FzBn6Yv8Xw0Ri5TaQbJcKd7LuE";
    let challenge = format!("{part_a}{part_b}{part_c}");
    assert_eq!(challenge.len(), 128);
    let query = AuthorizeQuery {
        code_challenge: Some(challenge.to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_code_challenge_129_chars_rejected() {
    let part_a = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let part_b = "QmN8vTx3YpRaLs6Uc0IwHbDfKgEoAi7Zn5Vy2XrJlM";
    let part_c = "q9Wt4SuGhPkCe1Od3FzBn6Yv8Xw0Ri5TaQbJcKd7LuEf";
    let challenge = format!("{part_a}{part_b}{part_c}");
    assert_eq!(challenge.len(), 129);
    let query = AuthorizeQuery {
        code_challenge: Some(challenge),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("too long"));
}

#[test]
fn test_code_challenge_invalid_base64url_chars_rejected() {
    let query = AuthorizeQuery {
        code_challenge: Some("dBjftJeZ4CVP+mB92K27uhbUJU1p1r/wW1gFWFOEjXk".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("base64url"));
}

#[test]
fn test_code_challenge_with_equals_padding_rejected() {
    let query = AuthorizeQuery {
        code_challenge: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEj==".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("base64url"));
}

#[test]
fn test_code_challenge_all_same_char_rejected() {
    let challenge = "a".repeat(43);
    let query = AuthorizeQuery {
        code_challenge: Some(challenge),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("entropy"));
}

#[test]
fn test_code_challenge_repeating_pattern_2char_rejected() {
    let challenge = "ab".repeat(22);
    let query = AuthorizeQuery {
        code_challenge: Some(challenge[..43].to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("entropy"));
}

#[test]
fn test_code_challenge_repeating_pattern_3char_rejected() {
    let challenge = "abc".repeat(15);
    let query = AuthorizeQuery {
        code_challenge: Some(challenge[..43].to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("entropy"));
}

#[test]
fn test_code_challenge_repeating_pattern_4char_rejected() {
    let challenge = "abcd".repeat(11);
    let query = AuthorizeQuery {
        code_challenge: Some(challenge[..43].to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("entropy"));
}

#[test]
fn test_code_challenge_sequential_ascending_run_rejected() {
    let challenge = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG";
    assert_eq!(challenge.len(), 43);
    let query = AuthorizeQuery {
        code_challenge: Some(challenge.to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("entropy"));
}

#[test]
fn test_code_challenge_low_diversity_rejected() {
    let mut challenge = String::new();
    challenge.push_str("aaa");
    for _ in 0..40 {
        challenge.push('b');
    }
    let query = AuthorizeQuery {
        code_challenge: Some(challenge),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("entropy"));
}

#[test]
fn test_code_challenge_method_s256_accepted() {
    let query = valid_query();
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_code_challenge_method_plain_rejected() {
    let query = AuthorizeQuery {
        code_challenge_method: Some("plain".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("plain"));
}

#[test]
fn test_code_challenge_method_missing_rejected() {
    let query = AuthorizeQuery {
        code_challenge_method: None,
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("code_challenge_method is required"));
}

#[test]
fn test_code_challenge_method_unknown_rejected() {
    let query = AuthorizeQuery {
        code_challenge_method: Some("S512".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("S512"));
}

#[test]
fn test_display_valid_values_accepted() {
    for display_value in &["page", "popup", "touch", "wap"] {
        let query = AuthorizeQuery {
            display: Some(display_value.to_string()),
            ..valid_query()
        };
        assert!(
            validate_oauth_parameters(&query, &default_origin()).is_ok(),
            "display '{display_value}' should be accepted"
        );
    }
}

#[test]
fn test_display_invalid_value_rejected() {
    let query = AuthorizeQuery {
        display: Some("fullscreen".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("fullscreen"));
}

#[test]
fn test_prompt_valid_single_values_accepted() {
    for prompt_value in &["none", "login", "consent", "select_account"] {
        let query = AuthorizeQuery {
            prompt: Some(prompt_value.to_string()),
            ..valid_query()
        };
        assert!(
            validate_oauth_parameters(&query, &default_origin()).is_ok(),
            "prompt '{prompt_value}' should be accepted"
        );
    }
}

#[test]
fn test_prompt_valid_multiple_values_accepted() {
    let query = AuthorizeQuery {
        prompt: Some("login consent".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_prompt_invalid_value_rejected() {
    let query = AuthorizeQuery {
        prompt: Some("force".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("force"));
}

#[test]
fn test_prompt_mixed_valid_and_invalid_rejected() {
    let query = AuthorizeQuery {
        prompt: Some("login force".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("force"));
}

#[test]
fn test_max_age_zero_accepted() {
    let query = AuthorizeQuery {
        max_age: Some(0),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_max_age_positive_accepted() {
    let query = AuthorizeQuery {
        max_age: Some(3600),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_max_age_negative_rejected() {
    let query = AuthorizeQuery {
        max_age: Some(-1),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("max_age"));
}

#[test]
fn test_resource_valid_https_accepted() {
    let query = AuthorizeQuery {
        resource: Some("https://api.example.com/v1".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_resource_valid_http_accepted() {
    let query = AuthorizeQuery {
        resource: Some("http://api.example.com/v1".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_resource_with_fragment_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://api.example.com/v1#section".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("fragment"));
}

#[test]
fn test_resource_localhost_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://localhost/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_127_0_0_1_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://127.0.0.1/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_0_0_0_0_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://0.0.0.0/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_internal_domain_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://service.internal/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_local_domain_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://myhost.local/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_10_x_private_range_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://10.0.0.1/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_192_168_x_private_range_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://192.168.1.1/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_172_16_private_range_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://172.16.0.1/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_172_31_private_range_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://172.31.255.255/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_172_32_public_range_accepted() {
    let query = AuthorizeQuery {
        resource: Some("https://172.32.0.1/api".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &default_origin()).is_ok());
}

#[test]
fn test_resource_169_254_link_local_rejected() {
    let query = AuthorizeQuery {
        resource: Some("https://169.254.1.1/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_invalid_uri_rejected() {
    let query = AuthorizeQuery {
        resource: Some("not-a-uri".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("Invalid resource URI"));
}

#[test]
fn test_resource_ftp_scheme_rejected() {
    let query = AuthorizeQuery {
        resource: Some("ftp://files.example.com/data".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &default_origin()).unwrap_err();
    assert!(err.contains("https or http"));
}

#[test]
fn test_resource_self_origin_localhost_accepted() {
    let self_origin = origin_of("http://localhost:8080");
    let query = AuthorizeQuery {
        resource: Some("http://localhost:8080/api/v1/mcp/foo/mcp".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &self_origin).is_ok());
}

#[test]
fn test_resource_different_port_localhost_rejected() {
    let self_origin = origin_of("http://localhost:8080");
    let query = AuthorizeQuery {
        resource: Some("http://localhost:9999/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &self_origin).unwrap_err();
    assert!(err.contains("internal or private"));
}

#[test]
fn test_resource_self_origin_public_accepted() {
    let self_origin = origin_of("https://gateway.example.com");
    let query = AuthorizeQuery {
        resource: Some("https://gateway.example.com/api/v1/mcp/foo/mcp".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &self_origin).is_ok());
}

#[test]
fn test_resource_non_self_local_suffix_still_rejected_with_loopback_self() {
    let self_origin = origin_of("http://localhost:8080");
    let query = AuthorizeQuery {
        resource: Some("https://myhost.local/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &self_origin).unwrap_err();
    assert!(err.contains("internal or private"));
}

fn dual_origins(primary: &str, request: &str) -> SelfOrigins {
    let p = Url::parse(primary).expect("primary parses").origin();
    let r = Url::parse(request).expect("request parses").origin();
    SelfOrigins::new(p, r)
}

#[test]
fn test_resource_matches_request_origin_when_primary_differs() {
    // RFC 9728 dual-self-identity: gateway advertises 127.0.0.1 because the
    // client dialled via 127.0.0.1, while api_external_url is localhost.
    // The resource URI the client constructs from discovery uses 127.0.0.1
    // and must pass the self-origin carve-out even though primary differs.
    let origins = dual_origins("http://localhost:8080", "http://127.0.0.1:8080");
    let query = AuthorizeQuery {
        resource: Some("http://127.0.0.1:8080/api/v1/mcp/foo/mcp".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &origins).is_ok());
}

#[test]
fn test_resource_matches_primary_origin_when_request_differs() {
    let origins = dual_origins("https://gateway.example.com", "https://gateway.example.com");
    let query = AuthorizeQuery {
        resource: Some("https://gateway.example.com/api/v1/mcp/foo/mcp".to_string()),
        ..valid_query()
    };
    assert!(validate_oauth_parameters(&query, &origins).is_ok());
}

#[test]
fn test_resource_unrelated_loopback_rejected_even_with_dual_origins() {
    // Dual self-origins cover the gateway's own loopback ports. A different
    // loopback port is NOT in either self-origin, so the stricter address
    // rules below the carve-out must still reject it.
    let origins = dual_origins("http://localhost:8080", "http://127.0.0.1:8080");
    let query = AuthorizeQuery {
        resource: Some("http://localhost:9999/api".to_string()),
        ..valid_query()
    };
    let err = validate_oauth_parameters(&query, &origins).unwrap_err();
    assert!(err.contains("internal or private"));
}
