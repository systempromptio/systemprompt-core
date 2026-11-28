use chrono::Utc;
use systemprompt_device::client::{ConsumerOutcome, ManifestFetch};
use systemprompt_device::protocol::DeviceCredential;
use systemprompt_device::protocol::receipts::{build_receipt, plan_matches};
use systemprompt_identifiers::ConsumerInstallationId;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::feedback::ContentDigest;
use systemprompt_models::feedback::receipts::{
    ReadbackStatus, ReceiptAcknowledgement, RuntimeFileReadback,
};
use systemprompt_scripted_instance::{
    Event, ManifestFixture, PluginFixture, Route, ScriptedInstance,
};

use crate::consumer::enrolment;
use crate::support::signed_in;

#[tokio::test]
async fn a_published_plugin_is_planned_and_its_receipt_accepted() {
    let plugin = PluginFixture::new("scripted-skill")
        .file("SKILL.md", b"# Skill\n".to_vec())
        .published();
    let instance = ScriptedInstance::builder()
        .manifest(ManifestFixture::minimal().with_plugin(plugin))
        .start()
        .await
        .unwrap();
    let (client, session) = signed_in(&instance).await;
    let key = client.pubkey().await.unwrap();
    let ManifestFetch::Fetched { manifest, .. } = client
        .manifest(&session.jwt, &key, None, false)
        .await
        .unwrap()
    else {
        panic!("the first fetch carries a manifest");
    };
    let publication = manifest.signed().plugins[0]
        .publication
        .clone()
        .expect("a published plugin names its publication");
    let device = client
        .enrol_device(&session.jwt, &enrolment())
        .await
        .unwrap()
        .credential;

    let ConsumerOutcome::Acknowledged(plan) = client.installation_plan(&device, &publication).await
    else {
        panic!("the plan is served");
    };
    assert!(plan_matches(&plan, &publication, HostKind::Harness));
    assert!(
        plan.runtime_files
            .iter()
            .any(|file| file.path == "SKILL.md")
    );
    let readback: Vec<RuntimeFileReadback> = plan
        .runtime_files
        .iter()
        .map(|file| RuntimeFileReadback {
            path: file.path.clone(),
            digest: ContentDigest::of(&file.bytes),
            bytes: file.bytes.len() as u64,
            executable: file.executable,
            content_check: ReadbackStatus::Verified,
            mode_check: ReadbackStatus::Verified,
        })
        .collect();
    let receipt = build_receipt(
        &plan,
        &readback,
        ConsumerInstallationId::generate(),
        Utc::now(),
    )
    .unwrap();
    let ConsumerOutcome::Acknowledged(acknowledged) = client.post_receipt(&device, &receipt).await
    else {
        panic!("the receipt is acknowledged");
    };
    assert_eq!(
        acknowledged.acknowledgement,
        ReceiptAcknowledgement::Accepted
    );
    let record = instance.wait_for(Event::Route(Route::Plan)).await.unwrap();
    assert!(!record.unscripted);
}

#[tokio::test]
async fn an_unpublished_plugin_has_no_plan() {
    let instance = ScriptedInstance::builder()
        .manifest(ManifestFixture::minimal().with_plugin(PluginFixture::new("scripted-skill")))
        .start()
        .await
        .unwrap();
    let (client, session) = signed_in(&instance).await;
    let device: DeviceCredential = client
        .enrol_device(&session.jwt, &enrolment())
        .await
        .unwrap()
        .credential;
    let missing = reqwest::Client::new()
        .get(format!(
            "{}{}/scripted-skill-resource/publications/scripted-skill-publication/bundle?host=harness",
            instance.base_url(),
            systemprompt_models::ApiPaths::CONSUMER_RESOURCES
        ))
        .bearer_auth(device.expose())
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status().as_u16(), 404);
}
