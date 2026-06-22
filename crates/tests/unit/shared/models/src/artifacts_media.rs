//! Unit tests for media/presentation artifact builders.
//!
//! Tests cover the builder field round-trips, serde JSON shapes
//! (including `skip_serializing_if` behaviour and renamed keys), and the
//! [`Artifact`] schema emission for the audio, video, image, list, card,
//! chart, and dashboard artifacts.

use serde_json::json;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, SkillId, SourceId, TraceId};
use systemprompt_models::artifacts::audio::AudioArtifact;
use systemprompt_models::artifacts::card::{
    CardCta, CardSection, PresentationCardArtifact, PresentationCardResponse,
};
use systemprompt_models::artifacts::chart::{ChartArtifact, ChartDataset};
use systemprompt_models::artifacts::dashboard::DashboardArtifact;
use systemprompt_models::artifacts::image::ImageArtifact;
use systemprompt_models::artifacts::list::{ListArtifact, ListItem};
use systemprompt_models::artifacts::traits::Artifact;
use systemprompt_models::artifacts::types::{ArtifactType, AxisType, ChartType};
use systemprompt_models::execution::RequestContext;

const TEST_CONTEXT_ID: &str = "00000000-0000-4000-8000-000000000001";

fn test_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("sess-media"),
        TraceId::new("trace-media"),
        ContextId::new(TEST_CONTEXT_ID),
        AgentName::new("media-agent"),
    )
}

// ---------- AudioArtifact ----------

#[test]
fn audio_new_defaults() {
    let a = AudioArtifact::new("https://example.com/a.mp3");
    assert_eq!(a.artifact_type, "audio");
    assert_eq!(a.src, "https://example.com/a.mp3");
    assert!(a.mime_type.is_none());
    assert!(a.title.is_none());
    assert!(a.controls);
    assert!(!a.autoplay);
    assert!(!a.loop_playback);
    assert_eq!(AudioArtifact::ARTIFACT_TYPE_STR, "audio");
}

#[test]
fn audio_builder_chain() {
    let a = AudioArtifact::new("s")
        .with_mime_type("audio/mpeg")
        .with_title("Track")
        .with_artist("Artist")
        .with_artwork("art.png")
        .with_autoplay()
        .with_loop()
        .without_controls();
    assert_eq!(a.mime_type.as_deref(), Some("audio/mpeg"));
    assert_eq!(a.title.as_deref(), Some("Track"));
    assert_eq!(a.artist.as_deref(), Some("Artist"));
    assert_eq!(a.artwork.as_deref(), Some("art.png"));
    assert!(a.autoplay);
    assert!(a.loop_playback);
    assert!(!a.controls);
}

#[test]
fn audio_with_request_and_skill() {
    let a = AudioArtifact::new("s")
        .with_request(&test_context())
        .with_execution_id("exec-1")
        .with_skill(SkillId::new("skill-1"), "My Skill");
    // metadata is private/skipped; ensure builder returns a valid artifact.
    assert_eq!(a.artifact_type(), ArtifactType::Audio);
    assert_eq!(a.src, "s");
}

#[test]
fn audio_serde_renames_loop_and_skips_none() {
    let a = AudioArtifact::new("src.mp3").with_loop();
    let v = serde_json::to_value(&a).unwrap();
    assert_eq!(v["x-artifact-type"], "audio");
    assert_eq!(v["loop"], true);
    assert!(v.get("mime_type").is_none());
    assert!(v.get("title").is_none());
    // private metadata field is skipped from serialization.
    assert!(v.get("metadata").is_none());
}

#[test]
fn audio_schema_shape() {
    let a = AudioArtifact::new("s");
    let s = a.to_schema();
    assert_eq!(s["type"], "object");
    assert_eq!(s["x-artifact-type"], "audio");
    assert_eq!(s["required"], json!(["src"]));
    assert_eq!(s["properties"]["controls"]["default"], true);
}

#[test]
fn audio_roundtrip_deserialize() {
    let a = AudioArtifact::new("s").with_title("T").with_autoplay();
    let v = serde_json::to_value(&a).unwrap();
    let back: AudioArtifact = serde_json::from_value(v).unwrap();
    assert_eq!(back.src, "s");
    assert_eq!(back.title.as_deref(), Some("T"));
    assert!(back.autoplay);
}

// ---------- VideoArtifact ----------

#[test]
fn video_new_defaults() {
    let v = systemprompt_models::artifacts::video::VideoArtifact::new("v.mp4");
    assert_eq!(v.artifact_type, "video");
    assert_eq!(v.src, "v.mp4");
    assert!(v.controls);
    assert!(!v.autoplay);
    assert!(!v.muted);
}

#[test]
fn video_autoplay_forces_muted() {
    let v = systemprompt_models::artifacts::video::VideoArtifact::new("v.mp4").with_autoplay();
    assert!(v.autoplay);
    assert!(v.muted);
}

#[test]
fn video_builder_chain() {
    let v = systemprompt_models::artifacts::video::VideoArtifact::new("v.mp4")
        .with_mime_type("video/mp4")
        .with_poster("p.png")
        .with_caption("cap")
        .with_loop()
        .without_controls()
        .with_execution_id("e")
        .with_skill(SkillId::new("sk"), "Skill");
    assert_eq!(v.mime_type.as_deref(), Some("video/mp4"));
    assert_eq!(v.poster.as_deref(), Some("p.png"));
    assert_eq!(v.caption.as_deref(), Some("cap"));
    assert!(v.loop_playback);
    assert!(!v.controls);
    assert_eq!(v.artifact_type(), ArtifactType::Video);
}

#[test]
fn video_serde_and_schema() {
    let v = systemprompt_models::artifacts::video::VideoArtifact::new("v.mp4").with_caption("cap");
    let j = serde_json::to_value(&v).unwrap();
    assert_eq!(j["x-artifact-type"], "video");
    assert_eq!(j["caption"], "cap");
    assert!(j.get("poster").is_none());
    let s = v.to_schema();
    assert_eq!(s["x-artifact-type"], "video");
    assert_eq!(s["required"], json!(["src"]));
    assert_eq!(s["properties"]["muted"]["default"], false);
}

// ---------- ImageArtifact ----------

#[test]
fn image_new_and_builder() {
    let img = ImageArtifact::new("i.png")
        .with_alt("alt text")
        .with_caption("a caption")
        .with_dimensions(640, 480)
        .with_execution_id("e")
        .with_skill(SkillId::new("sk"), "Skill");
    assert_eq!(img.src, "i.png");
    assert_eq!(img.alt.as_deref(), Some("alt text"));
    assert_eq!(img.caption.as_deref(), Some("a caption"));
    assert_eq!(img.width, Some(640));
    assert_eq!(img.height, Some(480));
    assert_eq!(img.artifact_type(), ArtifactType::Image);
    assert_eq!(ImageArtifact::ARTIFACT_TYPE_STR, "image");
}

#[test]
fn image_serde_skips_none_and_schema() {
    let img = ImageArtifact::new("i.png");
    let v = serde_json::to_value(&img).unwrap();
    assert_eq!(v["x-artifact-type"], "image");
    assert!(v.get("alt").is_none());
    assert!(v.get("width").is_none());
    let s = img.to_schema();
    assert_eq!(s["x-artifact-type"], "image");
    assert_eq!(s["required"], json!(["src"]));
    assert_eq!(s["properties"]["width"]["type"], "integer");
}

#[test]
fn image_with_request_roundtrip() {
    let img = ImageArtifact::new("i.png").with_request(&test_context());
    let v = serde_json::to_value(&img).unwrap();
    let back: ImageArtifact = serde_json::from_value(v).unwrap();
    assert_eq!(back.src, "i.png");
}

// ---------- ListArtifact / ListItem ----------

#[test]
fn list_item_new_and_builder() {
    let item = ListItem::new("Title", "Summary", "https://x/y")
        .with_id("id-1")
        .with_uri("scheme://blog/slug")
        .with_slug("slug")
        .with_source_id(SourceId::new("src-1"))
        .with_category("cat")
        .with_description("desc");
    assert_eq!(item.title, "Title");
    assert_eq!(item.summary, "Summary");
    assert_eq!(item.link, "https://x/y");
    assert_eq!(item.id.as_deref(), Some("id-1"));
    assert_eq!(item.uri.as_deref(), Some("scheme://blog/slug"));
    assert_eq!(item.slug.as_deref(), Some("slug"));
    assert_eq!(item.source_id, Some(SourceId::new("src-1")));
    assert_eq!(item.category.as_deref(), Some("cat"));
    assert_eq!(item.description.as_deref(), Some("desc"));
}

#[test]
fn list_item_serde_skips_none() {
    let item = ListItem::new("T", "S", "L");
    let v = serde_json::to_value(&item).unwrap();
    assert_eq!(v["title"], "T");
    assert!(v.get("id").is_none());
    assert!(v.get("uri").is_none());
    assert!(v.get("source_id").is_none());
}

#[test]
fn list_default_is_empty() {
    let l = ListArtifact::default();
    assert_eq!(l.count, 0);
    assert!(l.items.is_empty());
    assert_eq!(l.artifact_type, "list");
}

#[test]
fn list_with_items_sets_count() {
    let l = ListArtifact::new()
        .with_items(vec![
            ListItem::new("a", "s", "l"),
            ListItem::new("b", "s", "l"),
        ])
        .with_execution_id("e")
        .with_skill(SkillId::new("sk"), "Skill");
    assert_eq!(l.count, 2);
    assert_eq!(l.items.len(), 2);
    assert_eq!(l.artifact_type(), ArtifactType::List);
}

#[test]
fn list_schema_shape() {
    let l = ListArtifact::new().with_request(&test_context());
    let s = l.to_schema();
    assert_eq!(s["x-artifact-type"], "list");
    assert_eq!(s["required"], json!(["items"]));
    assert_eq!(s["properties"]["count"]["type"], "integer");
}

#[test]
fn list_serde_roundtrip() {
    let l = ListArtifact::new().with_items(vec![ListItem::new("a", "s", "l")]);
    let v = serde_json::to_value(&l).unwrap();
    assert_eq!(v["count"], 1);
    let back: ListArtifact = serde_json::from_value(v).unwrap();
    assert_eq!(back.count, 1);
    assert_eq!(back.items[0].title, "a");
}

// ---------- PresentationCard ----------

#[test]
fn card_section_builder() {
    let s = CardSection::new("Heading", "Content").with_icon("star");
    assert_eq!(s.heading, "Heading");
    assert_eq!(s.content, "Content");
    assert_eq!(s.icon.as_deref(), Some("star"));
    let plain = CardSection::new("H", "C");
    let v = serde_json::to_value(&plain).unwrap();
    assert!(v.get("icon").is_none());
}

#[test]
fn card_cta_builder() {
    let cta = CardCta::new("id", "Label", "msg", "primary").with_icon("arrow");
    assert_eq!(cta.id, "id");
    assert_eq!(cta.label, "Label");
    assert_eq!(cta.message, "msg");
    assert_eq!(cta.variant, "primary");
    assert_eq!(cta.icon.as_deref(), Some("arrow"));
}

#[test]
fn card_new_defaults_theme() {
    let c = PresentationCardArtifact::new("My Card");
    assert_eq!(c.title, "My Card");
    assert_eq!(c.theme, "gradient");
    assert_eq!(c.artifact_type, "presentation_card");
    assert!(c.subtitle.is_none());
    assert!(c.sections.is_empty());
    assert!(c.ctas.is_empty());
    assert_eq!(
        PresentationCardArtifact::ARTIFACT_TYPE_STR,
        "presentation_card"
    );
}

#[test]
fn card_builder_chain() {
    let c = PresentationCardArtifact::new("Card")
        .with_subtitle("sub")
        .with_sections(vec![CardSection::new("h", "c")])
        .add_section(CardSection::new("h2", "c2"))
        .with_ctas(vec![CardCta::new("i", "l", "m", "v")])
        .add_cta(CardCta::new("i2", "l2", "m2", "v2"))
        .with_theme("dark")
        .with_request(&test_context())
        .with_execution_id("exec-9")
        .with_skill(SkillId::new("sk"), "Skill");
    assert_eq!(c.subtitle.as_deref(), Some("sub"));
    assert_eq!(c.sections.len(), 2);
    assert_eq!(c.ctas.len(), 2);
    assert_eq!(c.theme, "dark");
    assert_eq!(c.execution_id.as_deref(), Some("exec-9"));
    assert_eq!(c.skill_id, Some(SkillId::new("sk")));
    assert_eq!(c.skill_name.as_deref(), Some("Skill"));
    assert_eq!(c.artifact_type(), ArtifactType::PresentationCard);
}

#[test]
fn card_serde_skips_empty_ctas() {
    let c = PresentationCardArtifact::new("Card");
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(v["x-artifact-type"], "presentation_card");
    assert_eq!(v["theme"], "gradient");
    assert!(v.get("ctas").is_none());
    assert!(v.get("subtitle").is_none());
    assert!(v.get("execution_id").is_none());
}

#[test]
fn card_schema_carries_theme_hint() {
    let c = PresentationCardArtifact::new("Card").with_theme("neon");
    let s = c.to_schema();
    assert_eq!(s["x-artifact-type"], "presentation_card");
    assert_eq!(s["required"], json!(["title", "sections"]));
    assert_eq!(s["x-presentation-hints"]["theme"], "neon");
}

#[test]
fn card_response_default_and_serde() {
    let r = PresentationCardResponse::default();
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["x-artifact-type"], "");
    assert!(v.get("subtitle").is_none());
    assert!(v.get("ctas").is_none());
    // round-trip a populated response.
    let populated = PresentationCardResponse {
        artifact_type: "presentation_card".to_owned(),
        title: "T".to_owned(),
        subtitle: Some("s".to_owned()),
        sections: vec![CardSection::new("h", "c")],
        ctas: vec![CardCta::new("i", "l", "m", "v")],
        theme: "gradient".to_owned(),
        execution_id: Some("e".to_owned()),
        skill_id: Some(SkillId::new("sk")),
        skill_name: Some("S".to_owned()),
    };
    let pv = serde_json::to_value(&populated).unwrap();
    let back: PresentationCardResponse = serde_json::from_value(pv).unwrap();
    assert_eq!(back.title, "T");
    assert_eq!(back.subtitle.as_deref(), Some("s"));
    assert_eq!(back.ctas.len(), 1);
}

// ---------- ChartArtifact ----------

#[test]
fn chart_dataset_new() {
    let d = ChartDataset::new("Sales", vec![1.0, 2.0, 3.0]);
    assert_eq!(d.label, "Sales");
    assert_eq!(d.data, vec![1.0, 2.0, 3.0]);
}

#[test]
fn chart_builder_chain() {
    let c = ChartArtifact::new("Revenue", ChartType::Bar)
        .with_x_axis_labels(vec!["Q1".to_owned(), "Q2".to_owned()])
        .with_datasets(vec![ChartDataset::new("A", vec![1.0])])
        .add_dataset(ChartDataset::new("B", vec![2.0]))
        .with_x_axis_type(AxisType::Category)
        .with_y_axis_type(AxisType::Linear)
        .with_axes("Quarter", "USD")
        .with_request(&test_context())
        .with_execution_id("e")
        .with_skill(SkillId::new("sk"), "Skill");
    assert_eq!(c.labels, vec!["Q1", "Q2"]);
    assert_eq!(c.datasets.len(), 2);
    assert_eq!(c.artifact_type(), ArtifactType::Chart);
}

#[test]
fn chart_with_labels_alias() {
    let c =
        ChartArtifact::new("T", ChartType::Line).with_labels(vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(c.labels, vec!["a", "b"]);
}

#[test]
fn chart_schema_carries_hints() {
    let c = ChartArtifact::new("My Chart", ChartType::Bar).with_axes("X-axis", "Y-axis");
    let s = c.to_schema();
    assert_eq!(s["x-artifact-type"], "chart");
    assert_eq!(s["required"], json!(["labels", "datasets"]));
    assert_eq!(s["x-chart-hints"]["title"], "My Chart");
    assert_eq!(s["x-chart-hints"]["x_axis"]["label"], "X-axis");
    assert_eq!(s["x-chart-hints"]["y_axis"]["label"], "Y-axis");
}

#[test]
fn chart_serde_excludes_skipped_fields() {
    let c = ChartArtifact::new("T", ChartType::Pie)
        .with_datasets(vec![ChartDataset::new("d", vec![1.0, 2.0])]);
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(v["x-artifact-type"], "chart");
    assert!(v.get("title").is_none());
    assert!(v.get("chart_type").is_none());
    assert_eq!(v["datasets"][0]["label"], "d");
}

// ---------- DashboardArtifact ----------

#[test]
fn dashboard_new_and_builder() {
    let d = DashboardArtifact::new("Ops")
        .with_description("desc")
        .with_execution_id("e")
        .with_skill(SkillId::new("sk"), "Skill")
        .with_request(&test_context());
    assert_eq!(d.title, "Ops");
    assert_eq!(d.description.as_deref(), Some("desc"));
    assert!(d.sections.is_empty());
    assert_eq!(d.artifact_type(), ArtifactType::Dashboard);
    assert_eq!(DashboardArtifact::ARTIFACT_TYPE_STR, "dashboard");
}

#[test]
fn dashboard_serde_skips_none_description() {
    let d = DashboardArtifact::new("Ops");
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(v["x-artifact-type"], "dashboard");
    assert_eq!(v["title"], "Ops");
    assert!(v.get("description").is_none());
    assert!(v.get("hints").is_none());
    assert!(v.get("metadata").is_none());
}

#[test]
fn dashboard_schema_shape() {
    let d = DashboardArtifact::new("Ops");
    let s = d.to_schema();
    assert_eq!(s["x-artifact-type"], "dashboard");
    assert_eq!(s["required"], json!(["title", "sections"]));
    assert!(s["x-dashboard-hints"].is_object());
}
