use std::path::Path;

use systemprompt_extension::prelude::{
    ChangeSource, ConfigPlane, ConfigPlaneError, PlaneContext, PlaneSnapshot, PlaneTx,
    ResolvedEntities, ResolvedEntity,
};
use systemprompt_identifiers::{NotificationRuleId, NotificationStreamId};
use systemprompt_models::notifications::{
    AudienceSubject, NotificationsDoc, RuleAction, RuleDecl, RuleMatch, StreamAttention, StreamDecl,
};
use systemprompt_notifications::NotificationsPlane;
use systemprompt_traits::Dependencies;

use crate::harness::{Harness, stream_subscription};

type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

fn stream(id: &str) -> StreamDecl {
    StreamDecl {
        id: NotificationStreamId::try_new(id).expect("id"),
        name: id.to_owned(),
        description: None,
        audience: vec![AudienceSubject::Everyone],
        publishers: vec![AudienceSubject::Role("admin".to_owned())],
        attention: StreamAttention::default(),
        retention_days: None,
        coalesce_window_secs: None,
    }
}

fn rule(name: &str) -> RuleDecl {
    RuleDecl {
        id: NotificationRuleId::generate(),
        name: name.to_owned(),
        priority: 1,
        r#match: RuleMatch::default(),
        action: RuleAction::default(),
    }
}

fn write_doc(root: &Path, file: &str, doc: &NotificationsDoc) {
    let dir = root.join("notifications");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join(file), serde_yaml::to_string(doc).expect("yaml")).expect("write");
}

#[test]
fn no_declaration_files_leave_the_database_authoritative() {
    let root = tempfile::tempdir().expect("tmp");
    assert!(
        NotificationsPlane
            .load_declared(root.path())
            .expect("load")
            .is_none()
    );
    assert!(
        NotificationsPlane
            .sources(root.path())
            .expect("sources")
            .is_none()
    );
}

#[test]
fn declarations_are_keyed_by_kind_and_sourced_to_their_file() {
    let root = tempfile::tempdir().expect("tmp");
    let news = stream("news");
    write_doc(
        root.path(),
        "streams.yaml",
        &NotificationsDoc {
            streams: vec![news.clone()],
            ..NotificationsDoc::default()
        },
    );
    write_doc(
        root.path(),
        "subscriptions.yaml",
        &NotificationsDoc {
            subscriptions: vec![stream_subscription(AudienceSubject::Everyone, &news.id)],
            ..NotificationsDoc::default()
        },
    );
    let sources = NotificationsPlane
        .sources(root.path())
        .expect("sources")
        .expect("some");
    let keys: Vec<&String> = sources.keys().collect();
    assert_eq!(
        keys,
        vec!["stream/news", "subscription/everyone|stream:news"]
    );
    let declared = NotificationsPlane
        .load_declared(root.path())
        .expect("load")
        .expect("some");
    assert_eq!(declared.entries.len(), 2);
    let rendered = NotificationsPlane
        .render_declarations(&sources.into_iter().map(|(k, e)| (k, e.value)).collect())
        .expect("render");
    assert!(rendered.contains("news"), "{rendered}");
}

#[test]
fn an_unparseable_file_is_unreadable() {
    let root = tempfile::tempdir().expect("tmp");
    std::fs::create_dir_all(root.path().join("notifications")).expect("dir");
    std::fs::write(root.path().join("notifications/rules.yaml"), "rules: [[[").expect("write");
    let err = NotificationsPlane.load_declared(root.path()).unwrap_err();
    assert!(
        matches!(&err, ConfigPlaneError::Unreadable { path, .. } if path == "notifications/rules.yaml"),
        "{err:?}"
    );
}

#[test]
fn a_stream_declared_in_two_files_is_invalid() {
    let root = tempfile::tempdir().expect("tmp");
    let doc = NotificationsDoc {
        streams: vec![stream("news")],
        ..NotificationsDoc::default()
    };
    write_doc(root.path(), "streams.yaml", &doc);
    write_doc(root.path(), "rules.yaml", &doc);
    let err = NotificationsPlane.load_declared(root.path()).unwrap_err();
    assert!(matches!(err, ConfigPlaneError::Invalid { .. }), "{err:?}");
}

#[test]
fn rendering_an_unknown_kind_is_invalid() {
    let snapshot: PlaneSnapshot = [("widget/x".to_owned(), "{}".to_owned())]
        .into_iter()
        .collect();
    let err = NotificationsPlane
        .render_declarations(&snapshot)
        .unwrap_err();
    assert!(matches!(err, ConfigPlaneError::Invalid { .. }), "{err:?}");
}

async fn apply(h: &Harness, entries: Vec<(String, Option<String>)>) -> Result<(u64, u64), String> {
    let deps = Dependencies::new();
    let root = std::env::temp_dir();
    let ctx = PlaneContext::new(&root, &deps);
    let resolved: ResolvedEntities = entries
        .into_iter()
        .map(|(key, value)| {
            let source = ChangeSource::Code("notifications/streams.yaml".to_owned());
            (key, ResolvedEntity { value, source })
        })
        .collect();
    let mut tx: Tx = h.pool.write_pool().begin().await.expect("begin");
    let outcome = NotificationsPlane
        .write(&ctx, &mut PlaneTx::new(&mut tx), &resolved)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.expect("commit");
    Ok((outcome.updated as u64, outcome.deleted as u64))
}

async fn current(h: &Harness) -> (PlaneSnapshot, Option<String>) {
    let deps = Dependencies::new();
    let root = std::env::temp_dir();
    let ctx = PlaneContext::new(&root, &deps);
    let mut tx: Tx = h.pool.write_pool().begin().await.expect("begin");
    let snapshot = NotificationsPlane
        .read_current(&ctx, &mut PlaneTx::new(&mut tx))
        .await
        .expect("read");
    let export = NotificationsPlane
        .export(&ctx, &mut PlaneTx::new(&mut tx))
        .await
        .expect("export");
    (snapshot, export)
}

#[tokio::test]
async fn declared_entities_are_written_read_back_exported_and_deleted() {
    let h = Harness::new("notif_plane_rw").await;
    let news = stream("plane-news");
    let subscription = stream_subscription(AudienceSubject::Role("sales".to_owned()), &news.id);
    let sub_key = "subscription/role:sales|stream:plane-news".to_owned();
    let (empty, none) = current(&h).await;
    assert!(empty.is_empty());
    assert!(none.is_none());

    let plane_rule = rule("plane-rule");
    let rule_key = format!("rule/{}", plane_rule.id);
    let written = apply(
        &h,
        vec![
            (
                "stream/plane-news".to_owned(),
                Some(serde_json::to_string(&news).unwrap()),
            ),
            (
                rule_key.clone(),
                Some(serde_json::to_string(&plane_rule).unwrap()),
            ),
            (
                sub_key.clone(),
                Some(serde_json::to_string(&subscription).unwrap()),
            ),
        ],
    )
    .await
    .expect("write");
    assert_eq!(written, (3, 0));

    let (snapshot, export) = current(&h).await;
    let keys: Vec<&str> = snapshot.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec![rule_key.as_str(), "stream/plane-news", sub_key.as_str()]
    );
    let export = export.expect("export");
    assert!(
        export.contains("plane-news") && export.contains("plane-rule"),
        "{export}"
    );

    let deleted = apply(
        &h,
        vec![
            (sub_key, None),
            (rule_key, None),
            ("stream/plane-news".to_owned(), None),
        ],
    )
    .await
    .expect("delete");
    assert_eq!(deleted, (0, 3));
    assert!(current(&h).await.0.is_empty());
    h.finish().await;
}

#[tokio::test]
async fn malformed_keys_and_values_are_refused() {
    let h = Harness::new("notif_plane_bad").await;
    for (key, value) in [
        ("no-slash", Some("{}".to_owned())),
        ("widget/x", Some("{}".to_owned())),
        ("stream/x", Some("not json".to_owned())),
        ("subscription/role:x|stream:y", None),
    ] {
        apply(&h, vec![(key.to_owned(), value)])
            .await
            .expect_err(key);
    }
    assert!(current(&h).await.0.is_empty());
    h.finish().await;
}
