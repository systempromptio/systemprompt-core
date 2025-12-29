mod card;
mod dashboard;
mod list;
mod table;
mod text;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use systemprompt_models::a2a::Artifact;

pub use card::render as render_card;
pub use dashboard::render as render_dashboard;
pub use list::render as render_list;
pub use table::render as render_table;
pub use text::render as render_text;

pub fn render_artifact(artifact: &Artifact, area: Rect, buf: &mut Buffer, scroll_offset: usize) {
    let artifact_type = artifact.metadata.artifact_type.as_str();

    match artifact_type {
        "text" | "copy_paste_text" | "blog" => text::render(artifact, area, buf, scroll_offset),
        "table" => table::render(artifact, area, buf, scroll_offset),
        "list" => list::render(artifact, area, buf, scroll_offset),
        "presentation_card" | "research" => card::render(artifact, area, buf, scroll_offset),
        "dashboard" => dashboard::render(artifact, area, buf, scroll_offset),
        "chart" => text::render_chart_fallback(artifact, area, buf),
        _ => text::render_fallback(artifact, area, buf, scroll_offset),
    }
}
