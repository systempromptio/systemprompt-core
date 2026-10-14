import { escapeHtml } from "/assets/js/utils/escape.js";
import { fmtRelative } from "/assets/js/utils/format.js";
import { t } from "/assets/js/i18n.js";

// The catalog is computed in Rust (`InferenceCatalogPayload`); this only maps
// it to markup. A stale catalog still has its models, so it warns instead of
// emptying.
export function catalogView(snap) {
  const catalog = snap.inference_catalog || { providers: [], total: 0, desktop_models: [], freshness: {}, verdict: { tone: "unknown", code: "waiting" } };
  const freshness = catalog.freshness || {};
  const verdict = catalog.verdict || { tone: "unknown", code: "waiting" };
  return { catalog, freshness, verdict, tone: verdict.tone };
}

function providerGroup(p) {
  const issue = p.configured ? "" : (p.config_issue || t("status-proxy-provider-unconfigured") || "not configured");
  const chips = p.models.map((m) => `<span class="sp-chip">${escapeHtml(m)}</span>`).join("");
  return `
    <div class="sp-kpi-card__group"${p.configured ? "" : ' data-state="muted"'}>
      <div class="sp-kpi-card__group-head">
        <span class="sp-kpi-card__group-name">${escapeHtml(p.name)}</span>
        <span class="sp-kpi-card__group-surface">${escapeHtml(p.surface)}</span>
        <span class="sp-kpi-card__group-count">${p.models.length}</span>
      </div>
      ${issue ? `<p class="sp-kpi-card__group-issue">${escapeHtml(issue)}</p>` : ""}
      ${chips ? `<div class="sp-chip-list sp-kpi-card__chips">${chips}</div>` : ""}
    </div>`;
}

export function staleBadge(catalog, freshness, verdict) {
  if (verdict.code !== "stale") { return ""; }
  const ago = fmtRelative(Number(catalog.fetched_at_unix || 0));
  const text = t("status-proxy-endpoints-stale", { ago, error: freshness.error }) || `last updated ${ago} · ${freshness.error}`;
  return `<p class="sp-kpi-card__stale" title="${escapeHtml(freshness.error)}">${escapeHtml(text)}</p>`;
}

export function endpointsBody(catalog, verdict) {
  if (catalog.providers.length === 0) {
    const id = verdict.code === "waiting" ? "status-proxy-endpoints-unknown" : "status-proxy-endpoints-empty";
    const fallback = verdict.code === "waiting" ? "Waiting for the gateway's model catalog." : "The gateway serves no models.";
    return `<p class="sp-kpi-card__label">${escapeHtml(t(id) || fallback)}</p>`;
  }
  return `<div class="sp-kpi-card__groups">${catalog.providers.map(providerGroup).join("")}</div>`;
}

export function desktopLine(catalog) {
  const count = (catalog.desktop_models || []).length;
  const text = t("status-proxy-endpoints-desktop", { count }) || `Claude Desktop: Claude models only (${count})`;
  return `<button type="button" class="sp-btn-ghost" data-action="open-desktop">${escapeHtml(text)}</button>`;
}
