use super::*;

async fn assert_rejected_without_queueing(f: &Fixture, change: &AnalyticsChange) {
    assert!(f.repository.submit(&f.owner, change).await.is_err());
    let queued: i64 =
        sqlx::query_scalar("SELECT count(*) FROM analytics_fact_changes WHERE owner_id = $1")
            .bind(f.owner.as_str())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(queued, 0, "invalid evidence must be rejected atomically");
    let health = f.repository.health(&f.owner).await.unwrap();
    assert_eq!(health.pending, 0);
    assert_eq!(health.generation, 0);
}

fn request_change(id: &str) -> AnalyticsChange {
    let now = Utc::now();
    let request_key = key(AnalyticsFactKind::Request, id);
    AnalyticsChange {
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
                succeeded: true,
                spend: RecordedSpend::Known {
                    currency: "USD".to_owned(),
                    amount_micros: 20,
                },
                input_tokens: Some(10),
                output_tokens: Some(5),
                latency_micros: Some(100),
            }),
        },
    }
}

#[tokio::test]
async fn invalid_request_evidence_never_creates_queue_or_checkpoint_state() {
    let f = Fixture::new().await;

    let mut mismatched_time = request_change("request-time");
    mismatched_time.occurred_at += Duration::seconds(1);
    assert_rejected_without_queueing(&f, &mismatched_time).await;

    let mut invalid_currency = request_change("request-currency");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Request(request),
    } = &mut invalid_currency.operation
    else {
        unreachable!()
    };
    request.spend = RecordedSpend::Known {
        currency: "usd".to_owned(),
        amount_micros: 20,
    };
    assert_rejected_without_queueing(&f, &invalid_currency).await;

    let mut overflowing_tokens = request_change("request-overflow");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Request(request),
    } = &mut overflowing_tokens.operation
    else {
        unreachable!()
    };
    request.input_tokens = Some(u64::MAX);
    assert_rejected_without_queueing(&f, &overflowing_tokens).await;

    let mut control_source = request_change("request-control-source");
    control_source.key.source = "fixture\nforged".to_owned();
    assert_rejected_without_queueing(&f, &control_source).await;
}

#[tokio::test]
async fn invalid_artifact_links_and_bounds_are_rejected_before_persistence() {
    let f = Fixture::new().await;
    let now = Utc::now();
    let artifact_key = key(AnalyticsFactKind::Artifact, "artifact-invalid");
    let artifact = NormalizedArtifactFact {
        artifact_key: artifact_key.clone(),
        execution_id: "execution-1".to_owned(),
        invocation_key: Some(key(AnalyticsFactKind::Invocation, "invocation-1")),
        request_key: Some(key(AnalyticsFactKind::Request, "request-1")),
        occurred_at: now,
        consumer: InvocationConsumerIdentity::HistoricalUnknown,
        skill: None,
        tool_name: "lookup".to_owned(),
        server_name: None,
        artifact_type: "table".to_owned(),
        source: systemprompt_models::mcp::ExecutionSource::Gateway,
        correlation: systemprompt_models::mcp::Correlation::Exact,
        is_structured: true,
        has_ui_resource: false,
        succeeded: true,
        payload_bytes: Some(100),
        findings: 0,
    };
    let change = |artifact: NormalizedArtifactFact| AnalyticsChange {
        change_id: AnalyticsChangeId::generate(),
        key: artifact_key.clone(),
        revision: 1,
        occurred_at: now,
        recorded_at: now,
        operation: AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::Artifact(Box::new(artifact)),
        },
    };

    let mut empty_execution = artifact.clone();
    empty_execution.execution_id.clear();
    assert_rejected_without_queueing(&f, &change(empty_execution)).await;

    let mut wrong_invocation_kind = artifact.clone();
    wrong_invocation_kind.invocation_key = Some(key(AnalyticsFactKind::Request, "wrong-kind"));
    assert_rejected_without_queueing(&f, &change(wrong_invocation_kind)).await;

    let mut payload_overflow = artifact.clone();
    payload_overflow.payload_bytes = Some(u64::MAX);
    assert_rejected_without_queueing(&f, &change(payload_overflow)).await;

    let mut findings_overflow = artifact;
    findings_overflow.findings = u64::MAX;
    assert_rejected_without_queueing(&f, &change(findings_overflow)).await;
}

#[tokio::test]
async fn rejected_correction_preserves_the_last_applied_fact_revision() {
    let f = Fixture::new().await;
    let accepted = invocation("stable-invocation", 1);
    f.repository.submit(&f.owner, &accepted).await.unwrap();
    f.drain().await;

    let mut invalid = invocation("stable-invocation", 2);
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Invocation(invocation),
    } = &mut invalid.operation
    else {
        unreachable!()
    };
    invocation.latency_micros = Some(u64::MAX);
    assert!(f.repository.submit(&f.owner, &invalid).await.is_err());

    let stored = f
        .repository
        .get_fact(&f.owner, &accepted.key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.revision, 1);
    let Some(NormalizedAnalyticsFact::Invocation(fact)) = stored.fact else {
        panic!("accepted invocation fact remains present");
    };
    assert_eq!(fact.invocation_id.as_str(), "stable-invocation");
    let health = f.repository.health(&f.owner).await.unwrap();
    assert_eq!(health.pending, 0);
    assert_eq!(health.generation, 1);
}

fn assessment_change(id: &str) -> AnalyticsChange {
    let now = Utc::now();
    let assessment_key = key(AnalyticsFactKind::Assessment, id);
    AnalyticsChange {
        change_id: AnalyticsChangeId::generate(),
        key: assessment_key.clone(),
        revision: 1,
        occurred_at: now,
        recorded_at: now,
        operation: AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::Assessment(NormalizedAssessmentFact {
                conversation_key: AssessmentConversationKey {
                    source: "fixture".to_owned(),
                    id: AnalyticsFactId::new(format!("conversation-{id}")),
                },
                assessment_key,
                invocation_key: key(AnalyticsFactKind::Invocation, "assessment-invocation"),
                occurred_at: now,
                outcome: AssessmentOutcome::Scored {
                    score_millionths: 750_000,
                },
            }),
        },
    }
}

#[tokio::test]
async fn invalid_assessment_identities_are_rejected_without_queue_or_generation_changes() {
    let f = Fixture::new().await;

    let mut empty_conversation = assessment_change("empty-conversation");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Assessment(value),
    } = &mut empty_conversation.operation
    else {
        unreachable!()
    };
    value.conversation_key.id = AnalyticsFactId::new("");
    assert_rejected_without_queueing(&f, &empty_conversation).await;

    let mut controlled_conversation = assessment_change("controlled-conversation");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Assessment(value),
    } = &mut controlled_conversation.operation
    else {
        unreachable!()
    };
    value.conversation_key.source = "fixture\nforged".to_owned();
    assert_rejected_without_queueing(&f, &controlled_conversation).await;

    let mut mismatched_key = assessment_change("mismatched-assessment");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Assessment(value),
    } = &mut mismatched_key.operation
    else {
        unreachable!()
    };
    value.assessment_key = key(AnalyticsFactKind::Assessment, "different-assessment");
    assert_rejected_without_queueing(&f, &mismatched_key).await;

    let mut wrong_invocation_kind = assessment_change("wrong-assessment-invocation-kind");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Assessment(value),
    } = &mut wrong_invocation_kind.operation
    else {
        unreachable!()
    };
    value.invocation_key = key(AnalyticsFactKind::Request, "not-an-invocation");
    assert_rejected_without_queueing(&f, &wrong_invocation_kind).await;

    let accepted = assessment_change("accepted-assessment");
    let expected = match &accepted.operation {
        AnalyticsChangeOperation::Replace { fact } => fact.clone(),
        _ => unreachable!(),
    };
    f.repository.submit(&f.owner, &accepted).await.unwrap();
    f.drain().await;
    let stored = f
        .repository
        .get_fact(&f.owner, &accepted.key)
        .await
        .unwrap()
        .expect("valid assessment is committed");
    assert_eq!(stored.revision, 1);
    assert_eq!(stored.fact, Some(expected));
}

fn association_change(id: &str) -> AnalyticsChange {
    let now = Utc::now();
    let association_key = key(AnalyticsFactKind::ResourceAssociation, id);
    AnalyticsChange {
        change_id: AnalyticsChangeId::generate(),
        key: association_key.clone(),
        revision: 1,
        occurred_at: now,
        recorded_at: now,
        operation: AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::ResourceAssociation(NormalizedResourceAssociationFact {
                association_key,
                invocation_key: key(AnalyticsFactKind::Invocation, "association-invocation"),
                request_key: key(AnalyticsFactKind::Request, "association-request"),
                occurred_at: now,
                attribution: InvocationResourceAttribution::Verified {
                    resource_id: ManagedResourceId::generate(),
                    revision_id: ResourceRevisionId::generate(),
                },
            }),
        },
    }
}

#[tokio::test]
async fn invalid_resource_association_links_are_rejected_atomically() {
    let f = Fixture::new().await;

    let mut mismatched_key = association_change("mismatched-association");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::ResourceAssociation(value),
    } = &mut mismatched_key.operation
    else {
        unreachable!()
    };
    value.association_key = key(
        AnalyticsFactKind::ResourceAssociation,
        "different-association",
    );
    assert_rejected_without_queueing(&f, &mismatched_key).await;

    let mut wrong_invocation_kind = association_change("wrong-invocation-kind");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::ResourceAssociation(value),
    } = &mut wrong_invocation_kind.operation
    else {
        unreachable!()
    };
    value.invocation_key = key(AnalyticsFactKind::Request, "not-an-invocation");
    assert_rejected_without_queueing(&f, &wrong_invocation_kind).await;

    let mut wrong_request_kind = association_change("wrong-request-kind");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::ResourceAssociation(value),
    } = &mut wrong_request_kind.operation
    else {
        unreachable!()
    };
    value.request_key = key(AnalyticsFactKind::Invocation, "not-a-request");
    assert_rejected_without_queueing(&f, &wrong_request_kind).await;

    let mut invalid_reference = association_change("invalid-reference");
    let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::ResourceAssociation(value),
    } = &mut invalid_reference.operation
    else {
        unreachable!()
    };
    value.request_key.source.clear();
    assert_rejected_without_queueing(&f, &invalid_reference).await;

    let accepted = association_change("accepted-association");
    let expected = match &accepted.operation {
        AnalyticsChangeOperation::Replace { fact } => fact.clone(),
        _ => unreachable!(),
    };
    f.repository.submit(&f.owner, &accepted).await.unwrap();
    f.drain().await;
    let stored = f
        .repository
        .get_fact(&f.owner, &accepted.key)
        .await
        .unwrap()
        .expect("valid resource association is committed");
    assert_eq!(stored.revision, 1);
    assert_eq!(stored.fact, Some(expected));
}
