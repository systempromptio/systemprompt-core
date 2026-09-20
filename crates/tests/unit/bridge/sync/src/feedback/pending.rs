use super::*;

#[test]
fn pending_plan_survives_offline_restart_and_new_generation_supersedes_without_fabricated_receipt()
{
    use systemprompt_bridge::feedback::outbox::PendingInstallation;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("outbox.json");
    let outbox = Outbox::new(path.clone(), scope("device"));
    let original = PendingInstallation::new(
        publication(),
        EvaluatorClient::Codex,
        vec![dir.path().join("native-skill")],
    );
    let key = outbox.reserve_installation(original).unwrap();
    outbox.begin_installation_attempt(&key).unwrap();
    let restarted = Outbox::new(path, scope("device"));
    assert!(restarted.entries().unwrap().is_empty());
    assert_eq!(restarted.pending_installations().unwrap().len(), 1);
    let mut newer = publication();
    newer.generation = 2;
    newer.publication_id = PublicationId::new("newer");
    restarted
        .reserve_installation(PendingInstallation::new(
            newer,
            EvaluatorClient::Codex,
            vec![dir.path().join("native-skill")],
        ))
        .unwrap();
    let pending = restarted.pending_installations().unwrap();
    assert_eq!(
        pending
            .iter()
            .filter(|(_, pending)| pending.superseded)
            .count(),
        1
    );
    assert!(
        pending
            .iter()
            .any(|(_, pending)| pending.publication.generation == 2 && !pending.superseded)
    );
    assert!(restarted.complete_installation(&key).is_err());
}

#[test]
fn conflicting_replay_cannot_replace_a_reserved_installation_plan() {
    use systemprompt_bridge::feedback::outbox::PendingInstallation;

    let dir = tempfile::tempdir().unwrap();
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    let roots = vec![dir.path().join("original-native-skill")];
    let key = outbox
        .reserve_installation(PendingInstallation::new(
            publication(),
            EvaluatorClient::Codex,
            roots.clone(),
        ))
        .unwrap();

    let mut conflicting = publication();
    conflicting.bundle_digest =
        systemprompt_bridge::ids::Sha256Digest::try_new("1".repeat(64)).unwrap();
    assert!(matches!(
        outbox.reserve_installation(PendingInstallation::new(
            conflicting,
            EvaluatorClient::Codex,
            vec![dir.path().join("attacker-controlled-root")],
        )),
        Err(FeedbackError::Scope)
    ));

    let pending = outbox.pending_installations().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, key);
    assert_eq!(pending[0].1.roots, roots);
    assert_eq!(
        serde_json::to_value(&pending[0].1.publication).unwrap(),
        serde_json::to_value(publication()).unwrap(),
        "the original publication identity remains reserved"
    );
}

#[test]
fn a_full_installation_outbox_reclaims_only_superseded_work_and_preserves_active_work() {
    use systemprompt_bridge::feedback::outbox::PendingInstallation;

    let dir = tempfile::tempdir().unwrap();
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    let roots = vec![dir.path().join("native-skill")];

    let mut unrelated = publication();
    unrelated.resource_id = ManagedResourceId::new("unrelated-resource");
    unrelated.publication_id = PublicationId::new("unrelated-publication");
    let unrelated_key = outbox
        .reserve_installation(PendingInstallation::new(
            unrelated,
            EvaluatorClient::Codex,
            roots.clone(),
        ))
        .unwrap();
    for generation in 1..=511 {
        let mut publication = publication();
        publication.generation = generation;
        publication.publication_id = PublicationId::new(format!("publication-{generation}"));
        outbox
            .reserve_installation(PendingInstallation::new(
                publication,
                EvaluatorClient::Codex,
                roots.clone(),
            ))
            .unwrap();
    }

    let before = outbox.pending_installations().unwrap();
    let before_keys: std::collections::BTreeSet<_> =
        before.iter().map(|(key, _)| key.clone()).collect();
    let unrelated_before = serde_json::to_value(
        before
            .iter()
            .find(|(key, _)| key == &unrelated_key)
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();

    let mut latest = publication();
    latest.generation = 512;
    latest.publication_id = PublicationId::new("publication-512");
    outbox
        .reserve_installation(PendingInstallation::new(
            latest,
            EvaluatorClient::Codex,
            roots,
        ))
        .unwrap();

    let pending = outbox.pending_installations().unwrap();
    let after_keys: std::collections::BTreeSet<_> =
        pending.iter().map(|(key, _)| key.clone()).collect();
    let removed: Vec<_> = before_keys.difference(&after_keys).collect();
    assert_eq!(pending.len(), 512);
    assert_eq!(removed.len(), 1, "one superseded plan makes room");
    assert!(
        before
            .iter()
            .find(|(key, _)| key == removed[0])
            .unwrap()
            .1
            .superseded,
        "an active plan must never be reclaimed"
    );
    assert_eq!(
        serde_json::to_value(
            pending
                .iter()
                .find(|(key, _)| key == &unrelated_key)
                .unwrap()
                .1
                .clone(),
        )
        .unwrap(),
        unrelated_before,
        "the unrelated active plan is byte-for-byte unchanged"
    );
    assert!(
        pending
            .iter()
            .any(|(_, plan)| { plan.publication.generation == 512 && !plan.superseded })
    );

    let active_only = Outbox::new(dir.path().join("active-only.json"), scope("device"));
    for index in 0..512 {
        let mut publication = publication();
        publication.resource_id = ManagedResourceId::new(format!("active-resource-{index}"));
        publication.publication_id = PublicationId::new(format!("active-publication-{index}"));
        active_only
            .reserve_installation(PendingInstallation::new(
                publication,
                EvaluatorClient::Codex,
                vec![dir.path().join(format!("active-root-{index}"))],
            ))
            .unwrap();
    }
    let active_path = dir.path().join("active-only.json");
    let active_bytes = std::fs::read(&active_path).unwrap();
    let mut overflow = publication();
    overflow.resource_id = ManagedResourceId::new("overflow-resource");
    overflow.publication_id = PublicationId::new("overflow-publication");
    assert!(matches!(
        active_only.reserve_installation(PendingInstallation::new(
            overflow,
            EvaluatorClient::Codex,
            vec![dir.path().join("overflow-root")],
        )),
        Err(FeedbackError::Full)
    ));
    assert_eq!(std::fs::read(&active_path).unwrap(), active_bytes);
}
