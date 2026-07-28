# DashboardRenderer cannot render typed `DashboardSection` payloads

## Summary

`DashboardRenderer` parses sections from loose JSON keys, but the typed
`DashboardSection` model serializes different key names. Any `DashboardArtifact`
built through the typed builders renders every section body empty — only the
section titles survive.

A second, smaller mismatch: `ChartArtifact` marks `chart_type`, `title`, and
the axis labels `#[serde(skip)]`, so a typed chart always renders with the
renderer defaults (line chart, no axis labels) unless the caller separately
threads rendering hints.

## Where

- Renderer: `crates/domain/mcp/src/services/ui_renderer/templates/dashboard/section.rs`
- Typed model: `crates/shared/models/src/artifacts/dashboard/section.rs`
- Chart model: `crates/shared/models/src/artifacts/chart/mod.rs`

## Detail

`DashboardSection::from_json` resolves the section type from `value.get("type")`:

```rust
let section_type = value.get("type").and_then(JsonValue::as_str)
    .map_or(SectionType::Text, |s| match s.to_lowercase().as_str() {
        "metrics" | "kpi" => SectionType::Metrics,
        ...
    });
```

But the typed model serializes:

```json
{
  "section_id": "spine-metrics",
  "title": "Session at a glance",
  "section_type": "metrics_cards",
  "data": { "metrics": [ ... ] },
  "layout": { "width": "full", "order": 0 }
}
```

There is no `type` key, so every typed section falls through to
`SectionType::Text`, whose body lookup (`data.get("text")` /
`data.get("content")`) also misses → an empty `<p class="section-text">`.

Notably the per-type body renderers already anticipate the typed shape — each
one falls back to `data.get("data")` (`render_metrics`, `render_table`,
`render_status`, `render_list`) — so the *data* would resolve fine. Only the
type resolution was never given the same fallback, and the typed serde names
(`metrics_cards` etc., from `#[serde(rename_all = "snake_case")]` on
`SectionType`) were never added to the match.

## Repro

Serialize this and render it through `artifact_ui_resource`:

```rust
DashboardArtifact::new("Governance dashboard").with_sections(vec![
    DashboardSection::new("m", "Metrics", SectionType::MetricsCards)
        .with_data(json!({"metrics": [{"label": "Allowed", "value": 12}]}))?,
])
```

Observed: one section box titled "Metrics" with an empty body.
Expected: a metrics grid with one card.

(Found while building a demo tool in `systemprompt-demo` that emits one
artifact of every `CliArtifact` type: every other renderer handles its typed
payload — card/message/media via `typed::artifact_payload`, table/chart/list
because their loose-key lookups happen to match the typed field names.
Dashboard is the only variant whose typed form does not render.)

## Suggested fix

In `DashboardSection::from_json`:

1. Resolve the type from `value.get("type").or_else(|| value.get("section_type"))`.
2. Add the typed serde names to the match: `"metrics_cards"` → Metrics,
   `"timeline"` → (nearest existing, or Text), plus the already-matching
   `"table" | "chart" | "status" | "list"`.

That plus the existing `data.get("data")` fallbacks makes the typed model
render with no model-side changes and no effect on the loose-JSON path.

For the chart mismatch, either serialize `chart_type`/`title`/axis labels
(dropping the `#[serde(skip)]`) or have `ChartRenderer` read them from the
data part in addition to `metadata.rendering_hints`.
