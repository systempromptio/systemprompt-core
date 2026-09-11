use serde_json::json;
use systemprompt_loader::vertex_discovery::client;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn entry(publisher: &str, name: &str) -> serde_json::Value {
    json!({
        "name": format!("publishers/{publisher}/models/{name}"),
        "versionId": "001",
        "launchStage": "GA",
        "supportedActions": {},
        "openSourceCategory": "THIRD_PARTY_OWNED_OSS",
    })
}

async fn page(server: &MockServer, publisher: &str, token: Option<&str>, body: serde_json::Value) {
    let mut mock = Mock::given(method("GET"))
        .and(path(format!("/v1beta1/publishers/{publisher}/models")))
        .and(header("authorization", "Bearer test-token"))
        .and(query_param("pageSize", "300"));
    mock = match token {
        Some(token) => mock.and(query_param("pageToken", token)),
        None => mock.and(wiremock::matchers::query_param_is_missing("pageToken")),
    };
    mock.respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

// Why: a publisher with more than 300 models is the normal case for
// `publishers/google`, so a client that reads only the first page would make
// discovery silently depend on Google's ordering.
#[tokio::test]
async fn pagination_is_followed_until_the_token_clears() {
    let server = MockServer::start().await;
    page(
        &server,
        "qwen",
        None,
        json!({"publisherModels": [entry("qwen", "one-maas")], "nextPageToken": "page-2"}),
    )
    .await;
    page(
        &server,
        "qwen",
        Some("page-2"),
        json!({"publisherModels": [entry("qwen", "two-maas")]}),
    )
    .await;

    let models =
        client::list_publisher_models(&reqwest::Client::new(), &server.uri(), "test-token", "qwen")
            .await
            .expect("both pages are served");

    let names: Vec<&str> = models.iter().map(|model| model.model_name()).collect();
    assert_eq!(names, vec!["one-maas", "two-maas"]);
}

#[tokio::test]
async fn an_empty_next_page_token_ends_the_listing() {
    let server = MockServer::start().await;
    page(
        &server,
        "qwen",
        None,
        json!({"publisherModels": [entry("qwen", "one-maas")], "nextPageToken": ""}),
    )
    .await;

    let models =
        client::list_publisher_models(&reqwest::Client::new(), &server.uri(), "test-token", "qwen")
            .await
            .expect("an empty token is the end, not another page");
    assert_eq!(models.len(), 1);
}

// Why: entitlement is per publisher, so a 403 on one is the expected steady
// state for a project that has accepted some partner terms and not others. It
// must cost that publisher and nothing else.
#[tokio::test]
async fn a_forbidden_publisher_is_reported_and_the_rest_still_list() {
    let server = MockServer::start().await;
    page(
        &server,
        "qwen",
        None,
        json!({"publisherModels": [entry("qwen", "one-maas")]}),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/v1beta1/publishers/zai-org/models"))
        .respond_with(ResponseTemplate::new(403).set_body_string("permission denied"))
        .mount(&server)
        .await;
    page(
        &server,
        "moonshotai",
        None,
        json!({"publisherModels": [entry("moonshotai", "kimi-maas")]}),
    )
    .await;

    let publishers = vec![
        "qwen".to_string(),
        "zai-org".to_string(),
        "moonshotai".to_string(),
    ];
    let (models, failures) = client::list_all(
        &reqwest::Client::new(),
        &server.uri(),
        "test-token",
        "vertex-maas",
        &publishers,
    )
    .await;

    let names: Vec<&str> = models.iter().map(|model| model.model_name()).collect();
    assert_eq!(names, vec!["one-maas", "kimi-maas"]);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with("vertex-maas/zai-org: "),
        "{failures:?}"
    );
    assert!(failures[0].contains("403"), "{failures:?}");
    assert!(failures[0].contains("permission denied"), "{failures:?}");
}

#[tokio::test]
async fn an_unreadable_body_is_a_reason_not_a_panic() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let reason =
        client::list_publisher_models(&reqwest::Client::new(), &server.uri(), "test-token", "qwen")
            .await
            .expect_err("a non-JSON body cannot be read");
    assert!(reason.contains("unreadable body"), "{reason}");
}
