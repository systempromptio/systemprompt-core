//! Unit tests for ImageMetadata and ImageGenerationInfo

use systemprompt_core_files::{ImageGenerationInfo, ImageMetadata};

// ============================================================================
// ImageMetadata Tests
// ============================================================================

#[test]
fn test_image_metadata_new() {
    let meta = ImageMetadata::new();
    assert!(meta.width.is_none());
    assert!(meta.height.is_none());
    assert!(meta.alt_text.is_none());
    assert!(meta.description.is_none());
    assert!(meta.generation.is_none());
}

#[test]
fn test_image_metadata_default() {
    let meta = ImageMetadata::default();
    assert!(meta.width.is_none());
    assert!(meta.height.is_none());
    assert!(meta.alt_text.is_none());
    assert!(meta.description.is_none());
    assert!(meta.generation.is_none());
}

#[test]
fn test_image_metadata_with_dimensions() {
    let meta = ImageMetadata::new().with_dimensions(1920, 1080);
    assert_eq!(meta.width, Some(1920));
    assert_eq!(meta.height, Some(1080));
}

#[test]
fn test_image_metadata_with_alt_text() {
    let meta = ImageMetadata::new().with_alt_text("A beautiful sunset");
    assert_eq!(meta.alt_text, Some("A beautiful sunset".to_string()));
}

#[test]
fn test_image_metadata_with_description() {
    let meta = ImageMetadata::new().with_description("Detailed image description");
    assert_eq!(
        meta.description,
        Some("Detailed image description".to_string())
    );
}

#[test]
fn test_image_metadata_with_generation() {
    let gen = ImageGenerationInfo::new("test prompt", "dall-e-3", "openai");
    let meta = ImageMetadata::new().with_generation(gen);

    assert!(meta.generation.is_some());
    let gen_info = meta.generation.unwrap();
    assert_eq!(gen_info.prompt, "test prompt");
    assert_eq!(gen_info.model, "dall-e-3");
    assert_eq!(gen_info.provider, "openai");
}

#[test]
fn test_image_metadata_builder_chain() {
    let gen = ImageGenerationInfo::new("prompt", "model", "provider");
    let meta = ImageMetadata::new()
        .with_dimensions(512, 512)
        .with_alt_text("Alt text")
        .with_description("Description")
        .with_generation(gen);

    assert_eq!(meta.width, Some(512));
    assert_eq!(meta.height, Some(512));
    assert_eq!(meta.alt_text, Some("Alt text".to_string()));
    assert_eq!(meta.description, Some("Description".to_string()));
    assert!(meta.generation.is_some());
}

#[test]
fn test_image_metadata_clone() {
    let meta = ImageMetadata::new()
        .with_dimensions(100, 100)
        .with_alt_text("test");

    let cloned = meta.clone();
    assert_eq!(meta.width, cloned.width);
    assert_eq!(meta.height, cloned.height);
    assert_eq!(meta.alt_text, cloned.alt_text);
}

// ============================================================================
// ImageMetadata Serialization Tests
// ============================================================================

#[test]
fn test_image_metadata_serialize_empty() {
    let meta = ImageMetadata::new();
    let json = serde_json::to_string(&meta).unwrap();
    assert_eq!(json, "{}");
}

#[test]
fn test_image_metadata_serialize_with_dimensions() {
    let meta = ImageMetadata::new().with_dimensions(800, 600);
    let json = serde_json::to_string(&meta).unwrap();

    assert!(json.contains("\"width\":800"));
    assert!(json.contains("\"height\":600"));
}

#[test]
fn test_image_metadata_serialize_skip_none() {
    let meta = ImageMetadata::new().with_alt_text("test");
    let json = serde_json::to_string(&meta).unwrap();

    assert!(json.contains("alt_text"));
    assert!(!json.contains("width"));
    assert!(!json.contains("height"));
    assert!(!json.contains("description"));
    assert!(!json.contains("generation"));
}

#[test]
fn test_image_metadata_deserialize_empty() {
    let meta: ImageMetadata = serde_json::from_str("{}").unwrap();
    assert!(meta.width.is_none());
    assert!(meta.height.is_none());
    assert!(meta.alt_text.is_none());
    assert!(meta.description.is_none());
    assert!(meta.generation.is_none());
}

#[test]
fn test_image_metadata_deserialize_with_fields() {
    let json = r#"{"width":1024,"height":768,"alt_text":"test alt"}"#;
    let meta: ImageMetadata = serde_json::from_str(json).unwrap();

    assert_eq!(meta.width, Some(1024));
    assert_eq!(meta.height, Some(768));
    assert_eq!(meta.alt_text, Some("test alt".to_string()));
}

#[test]
fn test_image_metadata_roundtrip() {
    let meta = ImageMetadata::new()
        .with_dimensions(1280, 720)
        .with_alt_text("Round trip test")
        .with_description("Testing serialization");

    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: ImageMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(meta.width, deserialized.width);
    assert_eq!(meta.height, deserialized.height);
    assert_eq!(meta.alt_text, deserialized.alt_text);
    assert_eq!(meta.description, deserialized.description);
}

// ============================================================================
// ImageGenerationInfo Tests
// ============================================================================

#[test]
fn test_image_generation_info_new() {
    let gen = ImageGenerationInfo::new("A sunset over mountains", "dall-e-3", "openai");

    assert_eq!(gen.prompt, "A sunset over mountains");
    assert_eq!(gen.model, "dall-e-3");
    assert_eq!(gen.provider, "openai");
    assert!(gen.resolution.is_none());
    assert!(gen.aspect_ratio.is_none());
    assert!(gen.generation_time_ms.is_none());
    assert!(gen.cost_estimate.is_none());
    assert!(gen.request_id.is_none());
}

#[test]
fn test_image_generation_info_with_resolution() {
    let gen = ImageGenerationInfo::new("prompt", "model", "provider").with_resolution("1024x1024");

    assert_eq!(gen.resolution, Some("1024x1024".to_string()));
}

#[test]
fn test_image_generation_info_with_aspect_ratio() {
    let gen = ImageGenerationInfo::new("prompt", "model", "provider").with_aspect_ratio("16:9");

    assert_eq!(gen.aspect_ratio, Some("16:9".to_string()));
}

#[test]
fn test_image_generation_info_with_generation_time() {
    let gen = ImageGenerationInfo::new("prompt", "model", "provider").with_generation_time(5000);

    assert_eq!(gen.generation_time_ms, Some(5000));
}

#[test]
fn test_image_generation_info_with_cost_estimate() {
    let gen = ImageGenerationInfo::new("prompt", "model", "provider").with_cost_estimate(0.02);

    assert_eq!(gen.cost_estimate, Some(0.02));
}

#[test]
fn test_image_generation_info_with_request_id() {
    let gen =
        ImageGenerationInfo::new("prompt", "model", "provider").with_request_id("req_abc123");

    assert_eq!(gen.request_id, Some("req_abc123".to_string()));
}

#[test]
fn test_image_generation_info_builder_chain() {
    let gen = ImageGenerationInfo::new("Create a logo", "stable-diffusion", "stability")
        .with_resolution("512x512")
        .with_aspect_ratio("1:1")
        .with_generation_time(3000)
        .with_cost_estimate(0.01)
        .with_request_id("req_xyz789");

    assert_eq!(gen.prompt, "Create a logo");
    assert_eq!(gen.model, "stable-diffusion");
    assert_eq!(gen.provider, "stability");
    assert_eq!(gen.resolution, Some("512x512".to_string()));
    assert_eq!(gen.aspect_ratio, Some("1:1".to_string()));
    assert_eq!(gen.generation_time_ms, Some(3000));
    assert_eq!(gen.cost_estimate, Some(0.01));
    assert_eq!(gen.request_id, Some("req_xyz789".to_string()));
}

#[test]
fn test_image_generation_info_clone() {
    let gen = ImageGenerationInfo::new("prompt", "model", "provider")
        .with_resolution("1024x1024")
        .with_cost_estimate(0.05);

    let cloned = gen.clone();
    assert_eq!(gen.prompt, cloned.prompt);
    assert_eq!(gen.resolution, cloned.resolution);
    assert_eq!(gen.cost_estimate, cloned.cost_estimate);
}

// ============================================================================
// ImageGenerationInfo Serialization Tests
// ============================================================================

#[test]
fn test_image_generation_info_serialize_minimal() {
    let gen = ImageGenerationInfo::new("prompt text", "gpt-image", "openai");
    let json = serde_json::to_string(&gen).unwrap();

    assert!(json.contains("\"prompt\":\"prompt text\""));
    assert!(json.contains("\"model\":\"gpt-image\""));
    assert!(json.contains("\"provider\":\"openai\""));
}

#[test]
fn test_image_generation_info_serialize_skip_none() {
    let gen = ImageGenerationInfo::new("prompt", "model", "provider");
    let json = serde_json::to_string(&gen).unwrap();

    assert!(!json.contains("resolution"));
    assert!(!json.contains("aspect_ratio"));
    assert!(!json.contains("generation_time_ms"));
    assert!(!json.contains("cost_estimate"));
    assert!(!json.contains("request_id"));
}

#[test]
fn test_image_generation_info_serialize_with_optionals() {
    let gen = ImageGenerationInfo::new("prompt", "model", "provider")
        .with_resolution("2048x2048")
        .with_generation_time(10000);

    let json = serde_json::to_string(&gen).unwrap();

    assert!(json.contains("\"resolution\":\"2048x2048\""));
    assert!(json.contains("\"generation_time_ms\":10000"));
}

#[test]
fn test_image_generation_info_deserialize() {
    let json = r#"{
        "prompt": "test prompt",
        "model": "test-model",
        "provider": "test-provider",
        "resolution": "768x768"
    }"#;

    let gen: ImageGenerationInfo = serde_json::from_str(json).unwrap();

    assert_eq!(gen.prompt, "test prompt");
    assert_eq!(gen.model, "test-model");
    assert_eq!(gen.provider, "test-provider");
    assert_eq!(gen.resolution, Some("768x768".to_string()));
    assert!(gen.aspect_ratio.is_none());
}

#[test]
fn test_image_generation_info_roundtrip() {
    let gen = ImageGenerationInfo::new("Generate art", "midjourney", "discord")
        .with_resolution("1792x1024")
        .with_aspect_ratio("16:9")
        .with_generation_time(8500)
        .with_cost_estimate(0.10)
        .with_request_id("mj_12345");

    let json = serde_json::to_string(&gen).unwrap();
    let deserialized: ImageGenerationInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(gen.prompt, deserialized.prompt);
    assert_eq!(gen.model, deserialized.model);
    assert_eq!(gen.provider, deserialized.provider);
    assert_eq!(gen.resolution, deserialized.resolution);
    assert_eq!(gen.aspect_ratio, deserialized.aspect_ratio);
    assert_eq!(gen.generation_time_ms, deserialized.generation_time_ms);
    assert_eq!(gen.cost_estimate, deserialized.cost_estimate);
    assert_eq!(gen.request_id, deserialized.request_id);
}

// ============================================================================
// Combined Tests
// ============================================================================

#[test]
fn test_image_metadata_with_full_generation_info() {
    let gen = ImageGenerationInfo::new("A futuristic city", "dall-e-3", "openai")
        .with_resolution("1024x1024")
        .with_aspect_ratio("1:1")
        .with_generation_time(4500)
        .with_cost_estimate(0.04)
        .with_request_id("chatcmpl-abc123");

    let meta = ImageMetadata::new()
        .with_dimensions(1024, 1024)
        .with_alt_text("AI generated futuristic city")
        .with_description("A photorealistic image of a futuristic city with flying cars")
        .with_generation(gen);

    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: ImageMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(meta.width, deserialized.width);
    assert_eq!(meta.alt_text, deserialized.alt_text);
    assert!(deserialized.generation.is_some());

    let gen_info = deserialized.generation.unwrap();
    assert_eq!(gen_info.prompt, "A futuristic city");
    assert_eq!(gen_info.provider, "openai");
}
