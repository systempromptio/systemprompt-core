use super::*;

#[tokio::test]
async fn repeated_reordered_changes_and_tombstones_replace_without_resurrection() {
    let f = Fixture::new().await;
    let newest = invocation("i", 3);
    let receipt = f
        .repository
        .submit(&f.owner, &newest)
        .await
        .expect("new change");
    let mut retry = newest.clone();
    retry.change_id = AnalyticsChangeId::generate();
    retry.recorded_at += Duration::seconds(1);
    assert_eq!(
        f.repository
            .submit(&f.owner, &retry)
            .await
            .expect("identical natural-key retry")
            .change_id,
        receipt.change_id
    );
    f.drain().await;
    f.repository
        .submit(&f.owner, &invocation("i", 1))
        .await
        .expect("late old change");
    f.drain().await;
    assert_eq!(
        f.repository
            .get_fact(&f.owner, &newest.key)
            .await
            .expect("get")
            .expect("fact")
            .revision,
        3
    );
    let mut tombstone = newest.clone();
    tombstone.revision = 4;
    tombstone.change_id = AnalyticsChangeId::generate();
    tombstone.operation = AnalyticsChangeOperation::Tombstone;
    f.repository
        .submit(&f.owner, &tombstone)
        .await
        .expect("delete");
    f.drain().await;
    f.repository
        .submit(&f.owner, &invocation("i", 2))
        .await
        .expect("late correction");
    f.drain().await;
    let fact = f
        .repository
        .get_fact(&f.owner, &newest.key)
        .await
        .expect("get")
        .expect("tombstone");
    assert!(fact.fact.is_none());
    assert_eq!(fact.revision, 4);
    assert_eq!(
        f.repository
            .health(&f.owner)
            .await
            .expect("health")
            .generation,
        2
    );
}

#[tokio::test]
async fn conflicting_revision_and_embedded_identity_are_rejected() {
    let f = Fixture::new().await;
    let change = invocation("i", 1);
    f.repository
        .submit(&f.owner, &change)
        .await
        .expect("submit");
    let mut conflict = change.clone();
    conflict.change_id = AnalyticsChangeId::generate();
    conflict.operation = AnalyticsChangeOperation::Tombstone;
    assert!(f.repository.submit(&f.owner, &conflict).await.is_err());
    let mut forged = invocation("i", 2);
    forged.key.id = AnalyticsFactId::new("other");
    assert!(f.repository.submit(&f.owner, &forged).await.is_err());
    let overflow = invocation("large", u64::MAX);
    assert!(f.repository.submit(&f.owner, &overflow).await.is_err());
}

#[tokio::test]
async fn late_resource_correction_preserves_authenticated_identity() {
    let f = Fixture::new().await;
    let mut change = invocation("late-receipt", 1);
    let identity = InvocationConsumerIdentity::Authenticated {
        consumer_id: UserId::new("consumer-facts"),
        device_id: systemprompt_identifiers::DeviceId::try_new("device-facts")
            .expect("nonempty fixture device"),
        host: systemprompt_models::feedback::EvaluatorClient::Codex,
        session_id: systemprompt_identifiers::NativeSessionId::new("session-facts"),
    };
    if let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Invocation(value),
    } = &mut change.operation
    {
        value.consumer = identity.clone();
    }
    f.repository
        .submit(&f.owner, &change)
        .await
        .expect("unknown attribution");
    f.drain().await;
    change.change_id = AnalyticsChangeId::generate();
    change.revision = 2;
    if let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Invocation(value),
    } = &mut change.operation
    {
        value.attribution = InvocationResourceAttribution::Verified {
            resource_id: ManagedResourceId::generate(),
            revision_id: ResourceRevisionId::generate(),
        };
    }
    f.repository
        .submit(&f.owner, &change)
        .await
        .expect("late correction");
    f.drain().await;
    let fact = f
        .repository
        .get_fact(&f.owner, &change.key)
        .await
        .expect("get")
        .expect("fact");
    let Some(NormalizedAnalyticsFact::Invocation(value)) = fact.fact else {
        panic!("invocation");
    };
    assert_eq!(value.consumer, identity);
    assert!(matches!(
        value.attribution,
        InvocationResourceAttribution::Verified { .. }
    ));
    assert_eq!(
        f.repository
            .list_facts(&f.owner, None, 10)
            .await
            .expect("facts")
            .len(),
        1
    );
}
