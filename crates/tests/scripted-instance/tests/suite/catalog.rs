use systemprompt_models::providers::ClientFamily;
use systemprompt_scripted_instance::{ModelFixture, ScriptedInstance};

use crate::support::signed_in;

#[tokio::test]
async fn a_model_is_listed_with_the_window_its_fixture_advertises() {
    let instance = ScriptedInstance::builder()
        .models([
            ModelFixture::messages("m-wide").context_window(1_000_000),
            ModelFixture::chat("m-default"),
        ])
        .start()
        .await
        .unwrap();
    assert_eq!(instance.models()[0].context_window, Some(1_000_000));
    assert_eq!(instance.models()[1].context_window, None);

    let (client, session) = signed_in(&instance).await;
    let catalog = client.models(&session.jwt).await.unwrap();
    assert_eq!(
        catalog.get("m-wide").unwrap().max_input_tokens,
        Some(1_000_000)
    );
    assert_eq!(
        catalog.get("m-default").unwrap().max_input_tokens,
        Some(200_000)
    );

    let profile = client.profile(&session.jwt).await.unwrap();
    let windows: Vec<u32> = profile
        .model_limits
        .values()
        .map(|limits| limits.context_window)
        .collect();
    assert!(windows.contains(&1_000_000) && windows.contains(&200_000));
}

#[tokio::test]
async fn a_window_wider_than_the_profile_can_name_is_capped_there() {
    let instance = ScriptedInstance::builder()
        .models([ModelFixture::messages("m-vast").context_window(u64::MAX)])
        .start()
        .await
        .unwrap();
    let (client, session) = signed_in(&instance).await;
    let profile = client.profile(&session.jwt).await.unwrap();
    let limits = profile.model_limits.values().next().unwrap();
    assert_eq!(limits.context_window, u32::MAX);
}

#[tokio::test]
async fn the_profile_lists_one_configured_provider_per_client_family() {
    let instance = ScriptedInstance::builder()
        .models([
            ModelFixture::messages("m-a"),
            ModelFixture::chat("m-b"),
            ModelFixture::messages("m-c"),
        ])
        .start()
        .await
        .unwrap();
    let (client, session) = signed_in(&instance).await;
    let profile = client.profile(&session.jwt).await.unwrap();
    let served: Vec<(Vec<ClientFamily>, Vec<String>, bool)> = profile
        .providers
        .into_iter()
        .map(|provider| (provider.clients, provider.models, provider.configured))
        .collect();
    assert_eq!(
        served,
        [
            (
                vec![ClientFamily::Claude],
                vec!["m-a".to_owned(), "m-c".to_owned()],
                true
            ),
            (vec![ClientFamily::OpenAi], vec!["m-b".to_owned()], true),
        ]
    );
}
