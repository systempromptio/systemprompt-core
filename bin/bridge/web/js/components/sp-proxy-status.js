import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { fmtRelative, publishSectionState } from "/assets/js/utils/format.js";
import { t } from "/assets/js/i18n.js";
import { toneDot, toneSection, worstTone } from "/assets/js/utils/verdict.js";
import { runAction } from "/assets/js/utils/action.js";

function proxyView(proxy) {
  const verdict = proxy.verdict || { tone: "unknown", code: "unknown" };
  return {
    tone: verdict.tone,
    dot: toneDot(verdict.tone),
    value: proxy.governing ? String(proxy.latency_ms ?? "?") : "—",
    unit: proxy.governing ? "ms" : "",
    label: t(`proxy-state-${verdict.code}`) || "",
    reason: verdict.tone === "err" ? (proxy.error || "") : "",
  };
}

// The catalog is computed in Rust (`InferenceCatalogPayload`); this only maps
// it to a tone and markup. A stale catalog still has its models, so it warns
// instead of emptying.
function catalogView(snap) {
  const catalog = snap.inference_catalog || { providers: [], total: 0, desktop_models: [], freshness: { state: "unknown" } };
  const freshness = catalog.freshness || { state: "unknown" };
  const tone = freshness.state === "fresh" && catalog.total > 0 ? "ok" : (freshness.state === "unknown" ? "unknown" : "warn");
  return { catalog, freshness, tone };
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

function staleBadge(catalog, freshness) {
  if (freshness.state !== "stale") { return ""; }
  const ago = fmtRelative(Number(catalog.fetched_at_unix || 0));
  const text = t("status-proxy-endpoints-stale", { ago, error: freshness.error }) || `last updated ${ago} · ${freshness.error}`;
  return `<p class="sp-kpi-card__stale" title="${escapeHtml(freshness.error)}">${escapeHtml(text)}</p>`;
}

function endpointsBody(catalog, freshness) {
  if (catalog.providers.length === 0) {
    const id = freshness.state === "unknown" ? "status-proxy-endpoints-unknown" : "status-proxy-endpoints-empty";
    const fallback = freshness.state === "unknown" ? "Waiting for the gateway's model catalog." : "The gateway serves no models.";
    return `<p class="sp-kpi-card__label">${escapeHtml(t(id) || fallback)}</p>`;
  }
  return `<div class="sp-kpi-card__groups">${catalog.providers.map(providerGroup).join("")}</div>`;
}

function desktopLine(catalog) {
  const count = (catalog.desktop_models || []).length;
  const text = t("status-proxy-endpoints-desktop", { count }) || `Claude Desktop: Claude models only (${count})`;
  return `<button type="button" class="sp-btn-ghost" data-action="open-desktop">${escapeHtml(text)}</button>`;
}

export class SpProxyStatus extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.registerAction("reset-secret", (trigger) => {
      if (!window.confirm(t("status-proxy-reset-secret-confirm")
        || "Reset the local proxy secret? The bridge restarts and every agent must be repaired afterwards.")) {
        return;
      }
      runAction(trigger, {
        run: () => bridge.proxyResetSecret(),
        success: () => t("status-proxy-reset-secret-done") || "Secret reset — the bridge is restarting.",
        context: t("status-proxy-reset-secret") || "Reset local proxy secret",
      });
    });
    this.registerAction("open-desktop", (trigger) => {
      const drawer = document.querySelector("sp-agent-drawer");
      if (drawer) { drawer.open("detail", "claude-desktop", trigger); }
    });
  }

  onConnect() {
    this.useSnapshot((s) => { this.snapshot = s; });
  }

  render() {
    const snap = this.snapshot || {};
    const proxy = snap.local_proxy || {};
    const view = proxyView(proxy);
    const url = proxy.url || "";
    const { catalog, freshness, tone: epState } = catalogView(snap);

    const healthDetails = [
      ["url", escapeHtml(url || "(none)")],
      view.reason ? ["error", escapeHtml(view.reason)] : null,
      proxy.latency_ms != null ? ["latency", `${proxy.latency_ms} ms`] : null,
      ["state", escapeHtml(view.label)],
    ].filter(Boolean);

    return `
      <div class="sp-kpi-grid">
        <article class="sp-kpi-card" data-state="${view.tone}">
          <div class="sp-kpi-card__head">
            <span data-l10n-id="status-proxy-health">Health</span>
            <span class="sp-dot ${view.dot}" aria-hidden="true"></span>
          </div>
          <div class="sp-kpi-card__value">
            <span>${escapeHtml(view.value)}</span>
            ${view.unit ? `<span class="sp-kpi-card__unit">${escapeHtml(view.unit)}</span>` : ""}
          </div>
          <div class="sp-kpi-card__label">${escapeHtml(view.label)}</div>
          ${view.reason ? `<p class="sp-kpi-card__error">${escapeHtml(view.reason)}</p>` : ""}
          <details>
            <summary>Details</summary>
            <dl class="sp-kpi-card__details">
              ${healthDetails.map(([k, v]) => `<dt>${escapeHtml(k)}</dt><dd>${v}</dd>`).join("")}
            </dl>
          </details>
          <div class="sp-kpi-card__foot">
            <span class="sp-kpi-card__foot-meta">${escapeHtml(url || "no URL configured")}</span>
            ${view.tone === "err" ? `<button type="button" class="sp-btn-ghost" data-action="reset-secret">${escapeHtml(t("status-proxy-reset-secret") || "Reset local proxy secret")}</button>` : ""}
          </div>
        </article>

        <article class="sp-kpi-card" data-state="${epState}">
          <div class="sp-kpi-card__head">
            <span data-l10n-id="status-proxy-endpoints">Inference endpoints</span>
            <span class="sp-dot ${toneDot(epState)}" aria-hidden="true"></span>
          </div>
          <div class="sp-kpi-card__value">
            <span>${catalog.total}</span>
            <span class="sp-kpi-card__unit">${catalog.total === 1 ? "model" : "models"}</span>
          </div>
          ${staleBadge(catalog, freshness)}
          ${endpointsBody(catalog, freshness)}
          <div class="sp-kpi-card__foot">
            <span class="sp-kpi-card__foot-meta" data-l10n-id="status-proxy-endpoints-detail">Models the proxy serves, by provider.</span>
            ${catalog.providers.length ? desktopLine(catalog) : ""}
          </div>
        </article>
      </div>
    `;
  }

  afterRender() {
    const snap = this.snapshot || {};
    const proxy = snap.local_proxy || {};
    const view = proxyView(proxy);
    const { tone: epState } = catalogView(snap);
    const overall = worstTone(view.tone, epState);
    publishSectionState(this, overall, toneSection(overall));
  }
}

reactive(SpProxyStatus.prototype, ["snapshot"]);
customElements.define("sp-proxy-status", SpProxyStatus);
