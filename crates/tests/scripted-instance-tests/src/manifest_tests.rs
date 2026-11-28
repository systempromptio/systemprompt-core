use chrono::Utc;
use systemprompt_device::client::{ManifestFetch, open_manifest, verify};
use systemprompt_harness::extension::ExtensionPolicy;
use systemprompt_harness::sync::acceptance::accept;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_scripted_instance::ModelScript;

use crate::fixture::{MODEL, Signed};

#[tokio::test(flavor = "multi_thread")]
async fn the_signed_manifest_verifies_is_accepted_and_applies_its_extensions() {
    let signed = Signed::start(ModelScript::empty()).await;
    let key = signed
        .accounts
        .pinned_key(&signed.account)
        .unwrap()
        .expect("the manifest key was not pinned at login");
    let served = signed.client.pubkey().await.unwrap();
    assert_eq!(served.fingerprint(), key.fingerprint());

    let jwt = signed.jwt().await;
    let ManifestFetch::Fetched {
        envelope, manifest, ..
    } = signed
        .client
        .manifest(&jwt, &key, None, true)
        .await
        .unwrap()
    else {
        panic!("an unconditional manifest fetch answered 304");
    };
    verify(&envelope, &key).unwrap();
    let reopened = open_manifest(&envelope, &key).unwrap();
    assert_eq!(reopened.manifest_version, manifest.manifest_version);
    let user = signed.accounts.user_id(&signed.account).unwrap();
    accept(&manifest, user.as_ref(), None, Utc::now()).unwrap();

    let composition = signed.governed().await;
    let applied = composition
        .governance
        .applied()
        .expect("the instance is applied");
    assert!(
        !applied.gateway.models.is_empty(),
        "the applied gateway offers no model"
    );
    assert_eq!(
        ExtensionPolicy::enabled(&manifest),
        ExtensionPolicy::enabled_for(&manifest, HostKind::Harness)
    );
    composition.shutdown().await;
    signed.accounts.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_model_catalog_offers_the_chosen_model() {
    let signed = Signed::start(ModelScript::empty()).await;
    let catalog = signed.client.models(&signed.jwt().await).await.unwrap();
    let offered: Vec<&str> = catalog.data.iter().map(|model| model.id.as_str()).collect();
    assert_eq!(offered, [MODEL]);
    signed.accounts.shutdown();
}
