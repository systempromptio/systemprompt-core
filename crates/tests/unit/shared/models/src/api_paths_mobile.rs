use systemprompt_models::ApiPaths;

#[test]
fn the_association_documents_are_the_vendor_well_known_paths() {
    assert_eq!(
        ApiPaths::WELLKNOWN_APPLE_APP_SITE_ASSOCIATION,
        "/.well-known/apple-app-site-association"
    );
    assert_eq!(
        ApiPaths::WELLKNOWN_ASSETLINKS,
        "/.well-known/assetlinks.json"
    );
    for path in [
        ApiPaths::WELLKNOWN_APPLE_APP_SITE_ASSOCIATION,
        ApiPaths::WELLKNOWN_ASSETLINKS,
    ] {
        assert!(path.starts_with(&format!("{}/", ApiPaths::WELLKNOWN_BASE)));
    }
}

#[test]
fn the_push_token_route_is_on_the_notifications_surface() {
    assert_eq!(
        ApiPaths::NOTIFICATIONS_PUSH_TOKEN,
        "/api/v1/notifications/push-token"
    );
    assert_eq!(
        ApiPaths::notifications_relative(ApiPaths::NOTIFICATIONS_PUSH_TOKEN),
        "/push-token"
    );
    assert_eq!(
        ApiPaths::notifications_relative(ApiPaths::NOTIFICATIONS_SEEN),
        "/seen"
    );
}

#[test]
fn the_device_list_is_on_the_admin_notifications_surface() {
    assert_eq!(
        ApiPaths::ADMIN_NOTIFICATIONS_DEVICES,
        "/api/v1/admin/notifications/devices"
    );
    assert_eq!(
        ApiPaths::admin_notifications_relative(ApiPaths::ADMIN_NOTIFICATIONS_DEVICES),
        "/devices"
    );
    assert_eq!(
        ApiPaths::admin_relative(ApiPaths::ADMIN_NOTIFICATIONS_DEVICES),
        "/notifications/devices"
    );
}

#[test]
fn a_path_outside_a_surface_is_returned_whole() {
    assert_eq!(
        ApiPaths::notifications_relative(ApiPaths::BRAIN),
        ApiPaths::BRAIN
    );
    assert_eq!(
        ApiPaths::admin_notifications_relative(ApiPaths::NOTIFICATIONS_PUSH_TOKEN),
        ApiPaths::NOTIFICATIONS_PUSH_TOKEN
    );
}
