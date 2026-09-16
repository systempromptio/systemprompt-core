use super::*;

#[tokio::test]
async fn raw_reference_shared_request_failed_spend_and_conversation_denominators() {
    let f = Fixture::new().await;
    let now = Utc::now();
    let resource = ManagedResourceId::generate();
    let mut invocation_a = invocation("a", 1);
    if let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Invocation(value),
    } = &mut invocation_a.operation
    {
        value.attribution = InvocationResourceAttribution::Verified {
            resource_id: resource.clone(),
            revision_id: ResourceRevisionId::generate(),
        };
    }
    f.repository
        .submit(&f.owner, &invocation_a)
        .await
        .expect("invocation");
    for (id, spend) in [
        (
            "request",
            RecordedSpend::Known {
                currency: "USD".to_owned(),
                amount_micros: 123,
            },
        ),
        ("unknown", RecordedSpend::UnknownPricing),
    ] {
        let request_key = key(AnalyticsFactKind::Request, id);
        let change = AnalyticsChange {
            change_id: AnalyticsChangeId::generate(),
            key: request_key.clone(),
            revision: 1,
            occurred_at: now,
            recorded_at: now,
            operation: AnalyticsChangeOperation::Replace {
                fact: NormalizedAnalyticsFact::Request(NormalizedRequestFact {
                    request_key,
                    occurred_at: now,
                    consumer: InvocationConsumerIdentity::HistoricalUnknown,
                    succeeded: false,
                    spend,
                    input_tokens: None,
                    output_tokens: None,
                    latency_micros: Some(50),
                }),
            },
        };
        f.repository
            .submit(&f.owner, &change)
            .await
            .expect("request");
    }
    for id in ["association-1", "association-2"] {
        let association_key = key(AnalyticsFactKind::ResourceAssociation, id);
        let change = AnalyticsChange {
            change_id: AnalyticsChangeId::generate(),
            key: association_key.clone(),
            revision: 1,
            occurred_at: now,
            recorded_at: now,
            operation: AnalyticsChangeOperation::Replace {
                fact: NormalizedAnalyticsFact::ResourceAssociation(
                    NormalizedResourceAssociationFact {
                        association_key,
                        invocation_key: invocation_a.key.clone(),
                        request_key: key(AnalyticsFactKind::Request, "request"),
                        occurred_at: now,
                        attribution: InvocationResourceAttribution::Verified {
                            resource_id: resource.clone(),
                            revision_id: ResourceRevisionId::generate(),
                        },
                    },
                ),
            },
        };
        f.repository
            .submit(&f.owner, &change)
            .await
            .expect("association");
    }
    for id in ["assessment-1", "assessment-2"] {
        let assessment_key = key(AnalyticsFactKind::Assessment, id);
        let change = AnalyticsChange {
            change_id: AnalyticsChangeId::generate(),
            key: assessment_key.clone(),
            revision: 1,
            occurred_at: now,
            recorded_at: now,
            operation: AnalyticsChangeOperation::Replace {
                fact: NormalizedAnalyticsFact::Assessment(NormalizedAssessmentFact {
                    conversation_key: AssessmentConversationKey {
                        source: "fixture".to_owned(),
                        id: AnalyticsFactId::new("conversation"),
                    },
                    assessment_key,
                    invocation_key: invocation_a.key.clone(),
                    occurred_at: now,
                    outcome: AssessmentOutcome::Scored {
                        score_millionths: 800_000,
                    },
                }),
            },
        };
        f.repository
            .submit(&f.owner, &change)
            .await
            .expect("assessment");
    }
    f.drain().await;
    let totals = f
        .repository
        .reference_totals(
            &f.owner,
            now - Duration::hours(1),
            now + Duration::hours(1),
            None,
        )
        .await
        .expect("totals");
    assert_eq!(totals.requests, 2);
    assert_eq!(totals.priced_requests, 1);
    assert_eq!(totals.failed_requests, 2);
    assert_eq!(totals.assessed_conversations, 1);
    assert_eq!(totals.spend_by_currency["USD"], 123);
    let cohort = f
        .repository
        .reference_totals(
            &f.owner,
            now - Duration::hours(1),
            now + Duration::hours(1),
            Some(&resource),
        )
        .await
        .expect("cohort");
    assert_eq!(cohort.requests, 1);
    assert_eq!(cohort.spend_by_currency["USD"], 123);
    assert!(cohort.related_spend_non_additive);
}
