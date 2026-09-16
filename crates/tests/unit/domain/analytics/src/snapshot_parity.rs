use super::*;

#[tokio::test]
async fn shared_requests_match_raw_reference_and_remain_non_additive_across_resources() {
    let f = Fixture::new().await;
    let now = Utc::now();
    let a = ManagedResourceId::generate();
    let b = ManagedResourceId::generate();
    for change in [
        request("request", 1, now, 123),
        association("a1", "request", &a, now),
        association("a2", "request", &a, now),
        association("b", "request", &b, now),
    ] {
        f.repository
            .submit(&f.owner, &change)
            .await
            .expect("submit");
    }
    let snapshot = refresh(&f, now).await;
    let from = snapshot.from_day.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let to = snapshot.to_day.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let raw = f
        .repository
        .reference_totals(&f.owner, from, to, None)
        .await
        .expect("raw");
    assert_eq!(snapshot.metrics.requests, raw.requests);
    assert_eq!(snapshot.spend_by_currency, raw.spend_by_currency);
    assert_eq!(snapshot.metrics.failed_requests, 1);
    assert_eq!(snapshot.metrics.priced_requests, 1);
    for resource in [a, b] {
        let scope = repository(&f)
            .snapshot(&f.owner, Some(&resource), 30)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(scope.metrics.requests, 1);
        assert_eq!(scope.spend_by_currency["USD"], 123);
        assert!(scope.related_spend_non_additive);
    }
}

#[tokio::test]
async fn distinct_identities_span_days_and_histograms_merge_counts() {
    let f = Fixture::new().await;
    let now = Utc::now();
    for (id, at, cost) in [
        ("first", now - Duration::days(1), 10),
        ("second", now, 1000),
    ] {
        let mut change = request(id, 1, at, cost);
        identify(&mut change, "same-user");
        f.repository.submit(&f.owner, &change).await.unwrap();
    }
    let result = refresh(&f, now).await;
    assert_eq!(result.distinct_users, Some(1));
    assert_eq!(result.distinct_sessions, Some(1));
    assert_eq!(
        result.histogram.percentile_upper_bound_micros(95).unwrap(),
        Some(1023)
    );
    assert_eq!(result.metrics.latency_measured_requests, 2);
}

#[tokio::test]
async fn late_association_correction_and_tombstone_replace_old_contributions() {
    let f = Fixture::new().await;
    let now = Utc::now();
    let a = ManagedResourceId::generate();
    let b = ManagedResourceId::generate();
    f.repository
        .submit(
            &f.owner,
            &request("request", 1, now - Duration::days(2), 40),
        )
        .await
        .unwrap();
    refresh(&f, now).await;
    let mut link = association("link", "request", &a, now);
    f.repository.submit(&f.owner, &link).await.unwrap();
    refresh(&f, now).await;
    assert_eq!(
        repository(&f)
            .snapshot(&f.owner, Some(&a), 30)
            .await
            .unwrap()
            .unwrap()
            .metrics
            .requests,
        1
    );
    link.revision = 2;
    link.change_id = AnalyticsChangeId::generate();
    if let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::ResourceAssociation(value),
    } = &mut link.operation
    {
        value.attribution = InvocationResourceAttribution::Verified {
            resource_id: b.clone(),
            revision_id: ResourceRevisionId::generate(),
        };
    }
    f.repository.submit(&f.owner, &link).await.unwrap();
    refresh(&f, now).await;
    assert_eq!(
        repository(&f)
            .snapshot(&f.owner, Some(&a), 30)
            .await
            .unwrap()
            .unwrap()
            .metrics
            .requests,
        0
    );
    assert_eq!(
        repository(&f)
            .snapshot(&f.owner, Some(&b), 30)
            .await
            .unwrap()
            .unwrap()
            .metrics
            .requests,
        1
    );
    link.revision = 3;
    link.change_id = AnalyticsChangeId::generate();
    link.operation = AnalyticsChangeOperation::Tombstone;
    f.repository.submit(&f.owner, &link).await.unwrap();
    refresh(&f, now).await;
    assert_eq!(
        repository(&f)
            .snapshot(&f.owner, Some(&b), 30)
            .await
            .unwrap()
            .unwrap()
            .metrics
            .requests,
        0
    );
}

#[test]
fn geometric_histogram_rejects_versions_and_merges_without_averaging_percentiles() {
    let mut a = LatencyHistogram::default();
    for _ in 0..99 {
        a.record(1).unwrap();
    }
    let mut b = LatencyHistogram::default();
    b.record(1_000_000).unwrap();
    a.merge(&b).unwrap();
    assert_eq!(a.percentile_upper_bound_micros(95).unwrap(), Some(1));
    assert_eq!(
        a.percentile_upper_bound_micros(100).unwrap(),
        Some(1_048_575)
    );
    b.version = 2;
    assert!(a.merge(&b).is_err());
    assert!(a.percentile_upper_bound_micros(0).is_err());
}

#[tokio::test]
async fn unknown_pricing_and_latest_conversation_assessment_keep_independent_denominators() {
    let f = Fixture::new().await;
    let now = Utc::now();
    let mut change = request("unknown", 1, now, 0);
    if let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Request(value),
    } = &mut change.operation
    {
        value.spend = RecordedSpend::UnknownPricing;
        value.latency_micros = None;
        value.input_tokens = None;
    }
    f.repository.submit(&f.owner, &change).await.unwrap();
    for (index, outcome) in [
        (
            0,
            AssessmentOutcome::Scored {
                score_millionths: 500_000,
            },
        ),
        (1, AssessmentOutcome::Failed),
    ] {
        let at = now - Duration::days(1 - index);
        let key = key(
            AnalyticsFactKind::Assessment,
            &format!("assessment-{index}"),
        );
        let change = AnalyticsChange {
            change_id: AnalyticsChangeId::generate(),
            key: key.clone(),
            revision: 1,
            occurred_at: at,
            recorded_at: now,
            operation: AnalyticsChangeOperation::Replace {
                fact: NormalizedAnalyticsFact::Assessment(NormalizedAssessmentFact {
                    conversation_key: AssessmentConversationKey {
                        source: "fixture".to_owned(),
                        id: AnalyticsFactId::new("same-conversation"),
                    },
                    assessment_key: key,
                    invocation_key: super::key(AnalyticsFactKind::Invocation, "invocation"),
                    occurred_at: at,
                    outcome,
                }),
            },
        };
        f.repository.submit(&f.owner, &change).await.unwrap();
    }
    let snapshot = refresh(&f, now).await;
    assert_eq!(snapshot.metrics.requests, 1);
    assert_eq!(snapshot.metrics.priced_requests, 0);
    assert_eq!(snapshot.metrics.latency_measured_requests, 0);
    assert_eq!(snapshot.metrics.token_measured_requests, 0);
    assert_eq!(snapshot.metrics.assessment_conversations, 1);
    assert_eq!(snapshot.metrics.assessed_conversations, 0);
    assert_eq!(snapshot.metrics.failed_assessments, 1);
    let totals = f
        .repository
        .reference_totals(
            &f.owner,
            now - Duration::days(2),
            now + Duration::seconds(1),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        totals.assessment_conversations,
        snapshot.metrics.assessment_conversations
    );
    assert_eq!(
        totals.failed_assessments,
        snapshot.metrics.failed_assessments
    );
}
