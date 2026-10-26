use systemprompt_identifiers::PluginId;
use systemprompt_models::ApiPaths;

#[test]
fn bridge_paths_live_under_their_bases() {
    for path in [
        ApiPaths::BRIDGE_PUBKEY,
        ApiPaths::BRIDGE_MANIFEST,
        ApiPaths::BRIDGE_PLUGINS,
        ApiPaths::BRIDGE_PLUGIN_FILE_ROUTE,
        ApiPaths::BRIDGE_LATEST,
        ApiPaths::BRIDGE_DOWNLOAD,
        ApiPaths::BRIDGE_DOWNLOAD_ROUTE,
        ApiPaths::BRIDGE_WHOAMI,
        ApiPaths::BRIDGE_PROFILE,
        ApiPaths::BRIDGE_PROFILE_ENABLED_HOSTS,
        ApiPaths::BRIDGE_PROFILE_HOST_MODEL_FILTER,
        ApiPaths::BRIDGE_PROFILE_USAGE,
        ApiPaths::BRIDGE_DEVICE,
        ApiPaths::BRIDGE_HEARTBEAT,
        ApiPaths::BRIDGE_STREAM,
    ] {
        assert!(
            path.starts_with(&format!("{}/", ApiPaths::BRIDGE_BASE)),
            "{path}"
        );
    }
    for path in [
        ApiPaths::BRIDGE_AUTH_PAT,
        ApiPaths::BRIDGE_AUTH_SESSION,
        ApiPaths::BRIDGE_AUTH_SESSION_PAT,
        ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT,
        ApiPaths::BRIDGE_AUTH_CAPABILITIES,
        ApiPaths::BRIDGE_PERSONAL_KEYS,
    ] {
        assert!(
            path.starts_with(&format!("{}/", ApiPaths::GATEWAY_BASE)),
            "{path}"
        );
    }
    for path in [
        ApiPaths::CONSUMER_RECEIPTS,
        ApiPaths::CONSUMER_SESSION_BINDINGS,
        ApiPaths::CONSUMER_DEVICE_ENROLLMENT,
        ApiPaths::CONSUMER_BUNDLE_ROUTE,
    ] {
        assert!(
            path.starts_with(&format!("{}/", ApiPaths::API_V1)),
            "{path}"
        );
    }
    assert!(ApiPaths::PUBLIC_HOOKS_TRACK.starts_with(&format!("{}/", ApiPaths::PUBLIC_HOOKS)));
}

#[test]
fn relative_forms_strip_exactly_the_mount_prefix() {
    assert_eq!(
        ApiPaths::gateway_relative(ApiPaths::BRIDGE_MANIFEST),
        "/bridge/manifest"
    );
    assert_eq!(
        ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_PAT),
        "/auth/bridge/pat"
    );
    assert_eq!(ApiPaths::gateway_relative(ApiPaths::OTEL), ApiPaths::OTEL);
    assert_eq!(
        ApiPaths::api_v1_relative(ApiPaths::CONSUMER_RECEIPTS),
        "/consumer/receipts"
    );
    assert_eq!(ApiPaths::api_v1_relative(ApiPaths::OTEL), ApiPaths::OTEL);
}

#[test]
fn the_family_predicates_accept_the_base_and_its_children_only() {
    for (path, otel, mcp, hook) in [
        ("/otel", true, false, false),
        ("/otel/", true, false, false),
        ("/otel/v1/traces", true, false, false),
        ("/otelx", false, false, false),
        ("/mcp", false, true, false),
        ("/mcp/", false, true, false),
        ("/mcp/primary", false, true, false),
        ("/mcpx", false, false, false),
        ("/v1/mcp", false, false, false),
        ("/api/public/hooks", false, false, false),
        ("/api/public/hooks/", false, false, false),
        ("/api/public/hooks/track", false, false, true),
        ("/api/public/hooksx/track", false, false, false),
        ("/v1/messages", false, false, false),
    ] {
        assert_eq!(ApiPaths::is_otel(path), otel, "is_otel({path})");
        assert_eq!(ApiPaths::is_mcp(path), mcp, "is_mcp({path})");
        assert_eq!(
            ApiPaths::is_public_hook(path),
            hook,
            "is_public_hook({path})"
        );
    }
}

#[test]
fn url_builders_compose_from_the_constants() {
    assert_eq!(
        ApiPaths::bridge_plugin_file(&PluginId::try_new("acme").unwrap(), "hooks/hooks.json"),
        "/v1/bridge/plugins/acme/hooks/hooks.json"
    );
    assert_eq!(
        ApiPaths::bridge_plugin_archive(&PluginId::try_new("acme").unwrap(), "abc123"),
        "/v1/bridge/plugins/acme/archive/abc123"
    );
    assert_eq!(
        ApiPaths::bridge_download("darwin-arm64"),
        "/v1/bridge/download/darwin-arm64"
    );
    assert_eq!(
        ApiPaths::otel_under("https://gw.example.test/"),
        "https://gw.example.test/otel"
    );
    assert_eq!(
        ApiPaths::otel_under("http://127.0.0.1:48217"),
        "http://127.0.0.1:48217/otel"
    );
    assert_eq!(ApiPaths::mcp_server_slug("/mcp/primary/x"), Some("primary"));
    assert_eq!(ApiPaths::mcp_server_slug("/mcp//x"), None);
    assert_eq!(ApiPaths::mcp_server_slug("/mcpx/primary"), None);
}
