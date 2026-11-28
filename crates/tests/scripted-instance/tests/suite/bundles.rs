use systemprompt_device::client::ManifestFetch;
use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_RELPATH, PluginManifest};
use systemprompt_models::documents::content_hash;
use systemprompt_scripted_instance::{
    Event, ManifestFixture, ModelScript, PluginFixture, Route, ScriptedInstance,
};

use crate::support::{instance, signed_in};

const SKILL: &str = "skills/review/SKILL.md";
const SKILL_BYTES: &[u8] = b"# Review\n";

fn plugin() -> PluginFixture {
    PluginFixture::governance("scripted-plugin").file(SKILL, SKILL_BYTES)
}

async fn bundled() -> ScriptedInstance {
    ScriptedInstance::builder()
        .manifest(ManifestFixture::minimal().with_plugin(plugin()))
        .start()
        .await
        .unwrap()
}

#[tokio::test]
async fn the_manifest_lists_a_plugin_with_the_digests_of_the_files_served() {
    let instance = bundled().await;
    assert_eq!(instance.manifest().plugins(), [plugin()]);
    let (client, session) = signed_in(&instance).await;
    let key = client.pubkey().await.unwrap();
    let fetched = client
        .manifest(&session.jwt, &key, None, false)
        .await
        .unwrap();
    let ManifestFetch::Fetched { manifest, .. } = fetched else {
        panic!("the first fetch carries a manifest");
    };
    let listed = &manifest.signed().plugins;
    assert_eq!(listed.len(), 1);
    let entry = &listed[0];
    assert_eq!(entry.id, plugin().id);
    assert!(entry.hooks.governance);
    let paths: Vec<&str> = entry.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, [PLUGIN_MANIFEST_RELPATH, SKILL]);

    for file in &entry.files {
        let bytes = client
            .plugin_file(&session.jwt, &entry.id, &file.path)
            .await
            .unwrap();
        assert_eq!(file.sha256.as_str(), content_hash(&bytes).as_str());
        assert_eq!(file.size, bytes.len() as u64);
    }
    let skill = client
        .plugin_file(&session.jwt, &entry.id, SKILL)
        .await
        .unwrap();
    assert_eq!(skill, SKILL_BYTES);
    let root = client
        .plugin_file(&session.jwt, &entry.id, PLUGIN_MANIFEST_RELPATH)
        .await
        .unwrap();
    let root: PluginManifest = serde_json::from_slice(&root).unwrap();
    assert_eq!(
        (root.name, root.version),
        (entry.id.to_string(), entry.version.clone())
    );

    let record = instance
        .wait_for(Event::Route(Route::PluginBundle))
        .await
        .unwrap();
    assert!(!record.unscripted);
}

#[tokio::test]
async fn a_file_the_fixture_does_not_hold_is_not_found() {
    let instance = bundled().await;
    let (client, session) = signed_in(&instance).await;
    let missing = client
        .plugin_file(&session.jwt, &plugin().id, "skills/none/SKILL.md")
        .await
        .unwrap_err();
    assert_eq!(missing.status(), Some(404));
    let stranger = PluginFixture::new("another-plugin");
    let unknown = client
        .plugin_file(&session.jwt, &stranger.id, SKILL)
        .await
        .unwrap_err();
    assert_eq!(unknown.status(), Some(404));
}

#[tokio::test]
async fn a_plugin_added_twice_is_listed_once_with_its_last_form() {
    let fixture = ManifestFixture::minimal()
        .with_plugin(PluginFixture::new("scripted-plugin"))
        .with_plugin(plugin());
    assert_eq!(fixture.plugins(), [plugin()]);
}

#[tokio::test]
async fn the_minimal_manifest_lists_no_plugin() {
    let instance = instance(ModelScript::empty()).await;
    assert!(instance.manifest().plugins().is_empty());
}
