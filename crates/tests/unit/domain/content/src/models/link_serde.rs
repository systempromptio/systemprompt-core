use systemprompt_content::models::{DestinationType, LinkType, UtmParams};

#[test]
fn utm_params_deserialize_full() {
    let json = r#"{
        "source": "google",
        "medium": "cpc",
        "campaign": "spring",
        "term": "rust",
        "content": "banner"
    }"#;
    let params: UtmParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.source, Some("google".to_string()));
    assert_eq!(params.medium, Some("cpc".to_string()));
    assert_eq!(params.campaign, Some("spring".to_string()));
    assert_eq!(params.term, Some("rust".to_string()));
    assert_eq!(params.content, Some("banner".to_string()));
}

#[test]
fn utm_params_deserialize_partial() {
    let json = r#"{"source": "twitter"}"#;
    let params: UtmParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.source, Some("twitter".to_string()));
    assert!(params.medium.is_none());
    assert!(params.campaign.is_none());
}

#[test]
fn utm_params_deserialize_empty_object() {
    let json = r#"{}"#;
    let params: UtmParams = serde_json::from_str(json).unwrap();
    assert!(params.source.is_none());
    assert!(params.medium.is_none());
    assert!(params.campaign.is_none());
    assert!(params.term.is_none());
    assert!(params.content.is_none());
}

#[test]
fn utm_params_serde_roundtrip() {
    let original = UtmParams {
        source: Some("email".to_string()),
        medium: Some("newsletter".to_string()),
        campaign: Some("weekly-digest".to_string()),
        term: None,
        content: Some("cta-button".to_string()),
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: UtmParams = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.source, original.source);
    assert_eq!(restored.medium, original.medium);
    assert_eq!(restored.campaign, original.campaign);
    assert_eq!(restored.term, original.term);
    assert_eq!(restored.content, original.content);
}

#[test]
fn link_type_serde_roundtrip() {
    for variant in [LinkType::Redirect, LinkType::Utm, LinkType::Both] {
        let json = serde_json::to_string(&variant).unwrap();
        let restored: LinkType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.as_str(), variant.as_str());
    }
}

#[test]
fn destination_type_serde_roundtrip() {
    for variant in [DestinationType::Internal, DestinationType::External] {
        let json = serde_json::to_string(&variant).unwrap();
        let restored: DestinationType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.as_str(), variant.as_str());
    }
}

#[test]
fn link_type_copy_semantics() {
    let original = LinkType::Redirect;
    let copied = original;
    assert_eq!(original.as_str(), copied.as_str());
}

#[test]
fn destination_type_copy_semantics() {
    let original = DestinationType::External;
    let copied = original;
    assert_eq!(original.as_str(), copied.as_str());
}
