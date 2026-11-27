use std::sync::Arc;

use sp_extension::{SpExtension, SpExtensions};
use sp_notifications::{FeedStatus, NotificationsExtension, NotificationsHandle};

use crate::support::{applied, context, enabled};

const INSTANCE: &str = "https://sp.example.com";

#[test]
fn the_extension_is_named_notifications_and_optional() {
    let metadata = NotificationsExtension::default().metadata();
    assert_eq!(metadata.id.as_str(), "notifications");
    assert!(!metadata.required);
    assert_eq!(metadata.dependencies, Vec::new());
}

#[tokio::test]
async fn the_handle_follows_only_an_instance_that_enables_it() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let extension: Arc<dyn SpExtension> = Arc::new(NotificationsExtension::default());
    let extensions = SpExtensions::from_extensions([extension]).unwrap();
    extensions.provide(&ctx);
    let handle = ctx.handles.get::<NotificationsHandle>().unwrap();
    assert_eq!(handle.status(), FeedStatus::Withheld);
    let appliers = extensions.appliers(&ctx);

    appliers
        .apply(&applied(INSTANCE, Some(enabled(&["notifications"]))))
        .unwrap();
    assert_eq!(handle.status(), FeedStatus::Connecting);

    appliers
        .apply(&applied(INSTANCE, Some(enabled(&["drive"]))))
        .unwrap();
    assert_eq!(handle.status(), FeedStatus::Withheld);

    appliers
        .apply(&applied(INSTANCE, Some(enabled(&["notifications"]))))
        .unwrap();
    appliers.apply(&applied(INSTANCE, None)).unwrap();
    assert_eq!(handle.status(), FeedStatus::Withheld);

    appliers
        .apply(&applied(INSTANCE, Some(enabled(&["notifications"]))))
        .unwrap();
    appliers.clear();
    assert_eq!(handle.status(), FeedStatus::Withheld);
}

#[tokio::test]
async fn the_extension_contributes_the_feed_service() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let services = NotificationsExtension::default().services(&ctx);
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].name(), "notifications-feed");
}
