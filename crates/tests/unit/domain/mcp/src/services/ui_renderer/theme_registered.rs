// A theme set on the renderer registry (what `UiSlot::build` does with the
// one contributed theme) is applied to every artifact the registry renders,
// and an unthemed registry leaves the shell untouched (the negative control).

use systemprompt_mcp::services::ui_renderer::{ArtifactTheme, UiRendererRegistry};
use systemprompt_models::{A2aArtifact as Artifact, ArtifactMetadata, Part};

const HARNESS_TOKENS: &str = "--mcpui-accent: #ff00aa;";
const HARNESS_EXTRA_CSS: &str = ".harness-marker { display: none; }";

fn harness_theme() -> ArtifactTheme {
    ArtifactTheme::new(HARNESS_TOKENS).with_extra_css(HARNESS_EXTRA_CSS)
}

fn table_artifact() -> Artifact {
    let metadata = ArtifactMetadata::new(
        "table".to_owned(),
        systemprompt_identifiers::ContextId::generate(),
        systemprompt_identifiers::TaskId::generate(),
    );
    Artifact {
        id: systemprompt_identifiers::ArtifactId::generate(),
        title: None,
        description: None,
        parts: vec![Part::data(
            serde_json::json!({"columns": ["a"], "data": [{"a": 1}]}),
        )],
        extensions: vec![],
        metadata,
    }
}

#[test]
fn artifact_theme_carries_its_tokens_and_optional_extra_css() {
    let bare = ArtifactTheme::new(HARNESS_TOKENS);
    assert_eq!(bare.tokens, HARNESS_TOKENS);
    assert!(
        bare.extra_css.is_empty(),
        "a theme without extra CSS declares none"
    );

    let with_css = bare.with_extra_css(HARNESS_EXTRA_CSS);
    assert_eq!(with_css.extra_css, HARNESS_EXTRA_CSS);
    assert_eq!(
        with_css.tokens, HARNESS_TOKENS,
        "adding extra CSS leaves the tokens alone"
    );
}

#[test]
fn a_themed_registry_applies_its_theme_to_every_rendered_artifact() {
    let mut registry = UiRendererRegistry::with_builtins();
    registry.set_theme(harness_theme());
    assert_eq!(registry.theme().map(|t| t.tokens), Some(HARNESS_TOKENS));

    let html = registry.render(&table_artifact()).expect("render").html;

    assert!(
        html.contains(HARNESS_TOKENS),
        "the theme's tokens are declared in the rendered shell"
    );
    assert!(
        html.contains(HARNESS_EXTRA_CSS),
        "the theme's extra CSS is appended to the shell"
    );

    let tokens_at = html.find(HARNESS_TOKENS).expect("tokens present");
    let extra_at = html.find(HARNESS_EXTRA_CSS).expect("extra css present");
    let renderer_css_at = html.find(".table").unwrap_or(extra_at);
    assert!(
        tokens_at < extra_at,
        "tokens land in :root first so extra CSS can override them"
    );
    assert!(
        tokens_at < renderer_css_at,
        "the theme's tokens precede the renderer's stylesheet"
    );
    assert!(
        html[..tokens_at].contains(":root"),
        "the theme tokens are wrapped in a :root block"
    );
    assert!(
        extra_at < html.find("</head>").expect("head closes"),
        "the extra CSS stays in the document head"
    );
}

#[test]
fn an_unthemed_registry_declares_no_theme() {
    let registry = UiRendererRegistry::with_builtins();
    assert!(registry.theme().is_none());
    let html = registry.render(&table_artifact()).expect("render").html;
    assert!(!html.contains(HARNESS_TOKENS));
    assert!(!html.contains(HARNESS_EXTRA_CSS));
}
