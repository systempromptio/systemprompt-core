// Registering an `ArtifactTheme` for this test binary. Registration is
// compile-time via `inventory`, so a theme declared here is the theme for every
// artifact this binary renders — which is the only way to reach
// `active_theme`'s Some path and the theme-injection branches in the HTML shell
// builder.

use systemprompt_mcp::services::ui_renderer::templates::TableRenderer;
use systemprompt_mcp::services::ui_renderer::{ArtifactTheme, UiRenderer, active_theme};
use systemprompt_models::{A2aArtifact as Artifact, ArtifactMetadata, DataPart, Part};

const HARNESS_TOKENS: &str = "--mcpui-accent: #ff00aa;";
const HARNESS_EXTRA_CSS: &str = ".harness-marker { display: none; }";

fn harness_theme() -> ArtifactTheme {
    ArtifactTheme::new(HARNESS_TOKENS).with_extra_css(HARNESS_EXTRA_CSS)
}

systemprompt_mcp::register_artifact_theme!(harness_theme, name = "mcp_tests_harness");

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
        parts: vec![Part::Data(DataPart {
            data: serde_json::json!({"columns": ["a"], "data": [{"a": 1}]})
                .as_object()
                .cloned()
                .expect("object"),
        })],
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
fn active_theme_resolves_the_theme_registered_by_this_binary() {
    let theme = active_theme().expect("this binary registers a theme");
    assert_eq!(theme.tokens, HARNESS_TOKENS);
    assert_eq!(theme.extra_css, HARNESS_EXTRA_CSS);
}

#[tokio::test]
async fn a_registered_theme_is_injected_into_every_rendered_artifact() {
    let html = TableRenderer::new()
        .render(&table_artifact())
        .await
        .expect("render")
        .html;

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
    assert!(
        tokens_at < extra_at,
        "tokens land in :root first so extra CSS can override them"
    );
    assert!(
        html[..tokens_at].contains(":root"),
        "the theme tokens are wrapped in a :root block"
    );
}
