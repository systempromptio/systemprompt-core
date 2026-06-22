//! Tests for the image metadata / provenance value types.

use systemprompt_traits::{ImageGenerationInfo, ImageMetadata};

#[test]
fn metadata_new_is_all_none() {
    let m = ImageMetadata::new();
    assert!(m.width.is_none());
    assert!(m.height.is_none());
    assert!(m.alt_text.is_none());
    assert!(m.description.is_none());
    assert!(m.generation.is_none());
}

#[test]
fn metadata_default_matches_new() {
    let d = ImageMetadata::default();
    assert!(d.width.is_none());
    assert!(d.height.is_none());
    assert!(d.generation.is_none());
}

#[test]
fn metadata_builders_set_fields() {
    let m = ImageMetadata::new()
        .with_dimensions(1024, 768)
        .with_alt_text("a cat")
        .with_description("a photo of a cat");

    assert_eq!(m.width, Some(1024));
    assert_eq!(m.height, Some(768));
    assert_eq!(m.alt_text.as_deref(), Some("a cat"));
    assert_eq!(m.description.as_deref(), Some("a photo of a cat"));
}

#[test]
fn metadata_with_generation_attaches_provenance() {
    let info = ImageGenerationInfo::new("draw a fox", "imagen-3", "google");
    let m = ImageMetadata::new().with_generation(info);

    let g = m.generation.expect("generation should be set");
    assert_eq!(g.prompt, "draw a fox");
    assert_eq!(g.model, "imagen-3");
    assert_eq!(g.provider, "google");
}

#[test]
fn metadata_serialization_skips_none_fields() {
    let m = ImageMetadata::new().with_dimensions(800, 600);
    let json = serde_json::to_value(&m).expect("serialize");

    assert_eq!(json["width"], 800);
    assert_eq!(json["height"], 600);
    assert!(json.get("alt_text").is_none());
    assert!(json.get("description").is_none());
    assert!(json.get("generation").is_none());
}

#[test]
fn metadata_roundtrips_through_serde() {
    let info = ImageGenerationInfo::new("p", "m", "prov").with_resolution("1024x1024");
    let original = ImageMetadata::new()
        .with_dimensions(1024, 1024)
        .with_alt_text("alt")
        .with_generation(info);

    let json = serde_json::to_string(&original).expect("serialize");
    let parsed: ImageMetadata = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed.width, Some(1024));
    assert_eq!(parsed.alt_text.as_deref(), Some("alt"));
    assert_eq!(
        parsed.generation.expect("gen").resolution.as_deref(),
        Some("1024x1024")
    );
}

#[test]
fn metadata_deserializes_from_empty_object() {
    let m: ImageMetadata = serde_json::from_str("{}").expect("deserialize");
    assert!(m.width.is_none());
    assert!(m.generation.is_none());
}

#[test]
fn generation_info_new_leaves_optionals_unset() {
    let g = ImageGenerationInfo::new("prompt", "model", "provider");
    assert_eq!(g.prompt, "prompt");
    assert_eq!(g.model, "model");
    assert_eq!(g.provider, "provider");
    assert!(g.resolution.is_none());
    assert!(g.aspect_ratio.is_none());
    assert!(g.generation_time_ms.is_none());
    assert!(g.cost_estimate.is_none());
    assert!(g.request_id.is_none());
}

#[test]
fn generation_info_builders_set_every_field() {
    let g = ImageGenerationInfo::new("p", "m", "prov")
        .with_resolution("512x512")
        .with_aspect_ratio("1:1")
        .with_generation_time(1500)
        .with_cost_estimate(0.04)
        .with_request_id("req-123");

    assert_eq!(g.resolution.as_deref(), Some("512x512"));
    assert_eq!(g.aspect_ratio.as_deref(), Some("1:1"));
    assert_eq!(g.generation_time_ms, Some(1500));
    assert_eq!(g.cost_estimate, Some(0.04));
    assert_eq!(g.request_id.as_deref(), Some("req-123"));
}

#[test]
fn generation_info_serialization_skips_none() {
    let g = ImageGenerationInfo::new("p", "m", "prov");
    let json = serde_json::to_value(&g).expect("serialize");

    assert_eq!(json["prompt"], "p");
    assert_eq!(json["model"], "m");
    assert_eq!(json["provider"], "prov");
    assert!(json.get("resolution").is_none());
    assert!(json.get("cost_estimate").is_none());
    assert!(json.get("request_id").is_none());
}

#[test]
fn generation_info_roundtrips_through_serde() {
    let g = ImageGenerationInfo::new("p", "m", "prov")
        .with_generation_time(900)
        .with_cost_estimate(1.25);

    let json = serde_json::to_string(&g).expect("serialize");
    let parsed: ImageGenerationInfo = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed.generation_time_ms, Some(900));
    assert_eq!(parsed.cost_estimate, Some(1.25));
    assert_eq!(parsed.prompt, "p");
}
