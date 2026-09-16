//! Status polling preserves credential identity and exposes late attribution.
use crate::consumer_fixture::fixture;
use systemprompt_models::feedback::EvaluatorClient;
#[tokio::test]
async fn status_is_device_scoped_and_late_binding_updates_the_same_invocation() {
    let f = fixture().await;
    let foreign = fixture().await;
    let invocation = f.invocation();
    let token = &f.credential.credential;
    let unknown = f
        .repo
        .record_consumer_invocation(token, &invocation)
        .await
        .unwrap();
    assert!(unknown.receipt_id.is_none());
    let before = f
        .repo
        .consumer_invocation_status(token, &invocation.invocation_id, invocation.host)
        .await
        .unwrap();
    assert_eq!(before.version, unknown.version);
    assert!(
        f.repo
            .consumer_invocation_status(
                &foreign.credential.credential,
                &invocation.invocation_id,
                invocation.host
            )
            .await
            .is_err()
    );
    assert!(
        f.repo
            .consumer_invocation_status(
                token,
                &invocation.invocation_id,
                EvaluatorClient::ClaudeCode
            )
            .await
            .is_err()
    );
    let request = f.receipt_binding().await;
    let binding = f.repo.bind_consumer_session(token, &request).await.unwrap();
    assert_eq!(
        f.repo
            .consumer_session_status(token, &binding.id)
            .await
            .unwrap()
            .id,
        binding.id
    );
    assert!(
        f.repo
            .consumer_session_status(&foreign.credential.credential, &binding.id)
            .await
            .is_err()
    );
    let after = f
        .repo
        .consumer_invocation_status(token, &invocation.invocation_id, invocation.host)
        .await
        .unwrap();
    assert_eq!(after.receipt_id, Some(request.receipt_id));
    assert!(after.version > before.version);
    f.repo.revoke_consumer_credential(&f.cert).await.unwrap();
    assert!(
        f.repo
            .consumer_session_status(token, &binding.id)
            .await
            .is_err()
    );
    assert!(
        f.repo
            .consumer_invocation_status(token, &invocation.invocation_id, invocation.host)
            .await
            .is_err()
    );
}
