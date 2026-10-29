use systemprompt_extension::prelude::{
    ChangeSource, ConfigPlane, PlaneContext, PlaneTx, ResolvedEntities, ResolvedEntity,
};
use systemprompt_models::notifications::{
    Attention, AudienceSubject, StreamAttention, SubscriptionDecl,
};
use systemprompt_notifications::{NotificationsError, NotificationsPlane};
use systemprompt_traits::Dependencies;

use crate::harness::{CODE_SOURCE, Harness, stream_subscription};

type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

async fn gated_stream(h: &Harness) -> systemprompt_identifiers::NotificationStreamId {
    let lead = h.user("lead", &["sales_lead"]).await;
    h.declare_stream(
        "owned-news",
        vec![AudienceSubject::Role("sales".to_owned())],
        vec![AudienceSubject::User(lead)],
        StreamAttention::default(),
    )
    .await
}

#[tokio::test]
async fn a_console_write_onto_a_code_declared_subscription_is_refused_declared_by_config() {
    let h = Harness::new("notif_declared_by_config").await;
    let member = h.user("member", &["sales"]).await;
    let stream = gated_stream(&h).await;
    let declared = stream_subscription(AudienceSubject::User(member.clone()), &stream);
    h.declare_subscription(&declared).await;

    let refused = h
        .service
        .subscribe(
            &member,
            SubscriptionDecl {
                attention_ceiling: Some(Attention::Silent),
                ..declared.clone()
            },
        )
        .await
        .expect_err("a declared subscription is owned by configuration");
    assert!(
        matches!(refused, NotificationsError::DeclaredByConfig { .. }),
        "{refused:?}"
    );

    let views = h.service.subscriptions(&member).await.expect("list");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].source, CODE_SOURCE, "ownership never changes");
    assert_eq!(views[0].decl.attention_ceiling, None, "nothing was written");
    h.finish().await;
}

async fn plane_write(h: &Harness, key: &str, value: Option<String>) -> bool {
    let deps = Dependencies::new();
    let root = std::env::temp_dir();
    let ctx = PlaneContext::new(&root, &deps);
    let resolved: ResolvedEntities = [(
        key.to_owned(),
        ResolvedEntity {
            value,
            source: ChangeSource::Code("notifications/subscriptions.yaml".to_owned()),
        },
    )]
    .into_iter()
    .collect();
    let mut tx: Tx = h.pool.write_pool().begin().await.expect("begin");
    let written = NotificationsPlane
        .write(&ctx, &mut PlaneTx::new(&mut tx), &resolved)
        .await
        .is_ok();
    if written {
        tx.commit().await.expect("commit");
    }
    written
}

async fn plane_keys(h: &Harness) -> Vec<String> {
    let deps = Dependencies::new();
    let root = std::env::temp_dir();
    let ctx = PlaneContext::new(&root, &deps);
    let mut tx: Tx = h.pool.write_pool().begin().await.expect("begin");
    let snapshot = NotificationsPlane
        .read_current(&ctx, &mut PlaneTx::new(&mut tx))
        .await
        .expect("read current");
    snapshot.into_keys().collect()
}

#[tokio::test]
async fn the_config_plane_never_adopts_overwrites_or_deletes_a_console_subscription() {
    let h = Harness::new("notif_plane_console").await;
    let member = h.user("member", &["sales"]).await;
    let stream = gated_stream(&h).await;
    let own = stream_subscription(AudienceSubject::User(member.clone()), &stream);
    h.service
        .subscribe(&member, own.clone())
        .await
        .expect("console subscription");
    let key = format!("subscription/{}|stream:{stream}", own.subject);

    assert!(
        !plane_keys(&h).await.contains(&key),
        "a console row is not part of the declared state"
    );
    let declared = SubscriptionDecl {
        attention_ceiling: Some(Attention::Silent),
        ..own.clone()
    };
    assert!(
        !plane_write(
            &h,
            &key,
            Some(serde_json::to_string(&declared).expect("json"))
        )
        .await,
        "a declaration colliding with a console row is refused"
    );
    assert!(
        !plane_write(&h, &key, None).await,
        "a console row is never deleted by reconcile"
    );

    let views = h.service.subscriptions(&member).await.expect("list");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].source, format!("console:{member}"));
    assert_eq!(views[0].decl.attention_ceiling, None);
    h.finish().await;
}
