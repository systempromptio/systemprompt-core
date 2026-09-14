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
