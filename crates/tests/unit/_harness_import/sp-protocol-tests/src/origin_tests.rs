use sp_protocol::OriginError;
use sp_protocol::origin::{
    DEFAULT_TOKEN_ENDPOINT_PATH, align_token_endpoint, bearer_allowed, instance_origin,
    is_device_link_redirect, is_transport_secure, normalize_instance_url, rehome_instance_mcp_url,
    resolve_instance_path,
};
use sp_protocol::wire::paths;
use url::Url;

fn url(raw: &str) -> Url {
    Url::parse(raw).unwrap()
}

#[test]
fn plain_http_only_reaches_loopback() {
    assert!(is_transport_secure(&url("https://example.com")));
    assert!(is_transport_secure(&url("http://127.0.0.1:8080")));
    assert!(is_transport_secure(&url("http://localhost:8080")));
    assert!(is_transport_secure(&url("http://[::1]:8080")));
    assert!(!is_transport_secure(&url("http://example.com")));
    assert!(!is_transport_secure(&url("http://127.0.0.2")));
    assert!(!is_transport_secure(&url("ftp://localhost")));
}

#[test]
fn instance_urls_normalize() {
    assert_eq!(
        normalize_instance_url("https://sp.example.com/").unwrap(),
        "https://sp.example.com"
    );
    assert_eq!(
        normalize_instance_url(" https://sp.example.com/base//?q=1#f ").unwrap(),
        "https://sp.example.com/base"
    );
    assert_eq!(
        normalize_instance_url("http://localhost:8080").unwrap(),
        "http://localhost:8080"
    );
    assert_eq!(
        normalize_instance_url("http://remote.example"),
        Err(OriginError::Insecure)
    );
    assert_eq!(
        normalize_instance_url("https://u:p@sp.example.com"),
        Err(OriginError::Credentials)
    );
    assert!(matches!(
        normalize_instance_url("not a url"),
        Err(OriginError::Parse(_))
    ));
    assert_eq!(
        instance_origin("https://sp.example.com/base").unwrap(),
        "https://sp.example.com"
    );
}

#[test]
fn advertised_paths_stay_on_the_origin() {
    let base = "https://sp.example.com/base";
    assert_eq!(
        resolve_instance_path(base, "/bridge-auth/device-link")
            .unwrap()
            .as_str(),
        "https://sp.example.com/base/bridge-auth/device-link"
    );
    assert_eq!(
        resolve_instance_path(base, "//evil.example/x"),
        Err(OriginError::Path)
    );
    assert_eq!(
        resolve_instance_path(base, "relative"),
        Err(OriginError::Path)
    );
    assert_eq!(resolve_instance_path(base, "/a\\b"), Err(OriginError::Path));
    assert_eq!(
        resolve_instance_path("https://sp.example.com", "@evil.example/x"),
        Err(OriginError::Path)
    );
}

#[test]
fn device_link_redirects() {
    assert!(is_device_link_redirect(&url(
        "http://127.0.0.1:53111/cb?state=x"
    )));
    assert!(is_device_link_redirect(&url("http://localhost:1/")));
    assert!(!is_device_link_redirect(&url("http://127.0.0.1/cb")));
    assert!(!is_device_link_redirect(&url("https://127.0.0.1:5000/cb")));
    assert!(!is_device_link_redirect(&url("http://[::1]:5000/cb")));
    assert!(!is_device_link_redirect(&url("http://127.0.0.2:5000/cb")));
}

#[test]
fn the_jwt_only_goes_to_the_instance_origin() {
    let instance = url("https://sp.example.com/base");
    assert!(bearer_allowed(
        &url("https://sp.example.com/api/v1/mcp/docs/mcp"),
        &instance
    ));
    assert!(!bearer_allowed(
        &url("https://other.example.com/mcp"),
        &instance
    ));
    assert!(!bearer_allowed(
        &url("http://sp.example.com/mcp"),
        &instance
    ));
}

#[test]
fn local_instance_servers_are_rehomed() {
    let instance = url("http://127.0.0.1:18080");
    let announced = url("http://localhost:8080/api/v1/mcp/docs/mcp?x=1");
    assert_eq!(
        rehome_instance_mcp_url(&announced, &instance).as_str(),
        "http://127.0.0.1:18080/api/v1/mcp/docs/mcp?x=1"
    );
    let other = url("http://localhost:9000/other/mcp");
    assert_eq!(rehome_instance_mcp_url(&other, &instance), other);
    let remote = url("https://sp.example.com/api/v1/mcp/docs/mcp");
    assert_eq!(rehome_instance_mcp_url(&remote, &instance), remote);
}

#[test]
fn token_endpoints_are_aligned_to_the_instance() {
    let instance = url("https://sp.example.com");
    assert_eq!(
        align_token_endpoint(
            Some("https://sp.example.com/api/v1/core/oauth/token"),
            &instance
        ),
        "https://sp.example.com/api/v1/core/oauth/token"
    );
    assert_eq!(
        align_token_endpoint(Some("http://localhost:8080/custom/token"), &instance),
        "https://sp.example.com/custom/token"
    );
    assert_eq!(
        align_token_endpoint(None, &instance),
        "https://sp.example.com/api/v1/core/oauth/token"
    );
    assert_eq!(
        align_token_endpoint(Some("::"), &instance),
        "https://sp.example.com/api/v1/core/oauth/token"
    );
}

#[test]
fn the_default_token_path_is_the_core_route() {
    assert_eq!(DEFAULT_TOKEN_ENDPOINT_PATH, paths::OAUTH_TOKEN);
    assert_eq!(DEFAULT_TOKEN_ENDPOINT_PATH, "/api/v1/core/oauth/token");
}
