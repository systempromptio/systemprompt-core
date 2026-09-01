import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { publishSectionState } from "/assets/js/utils/format.js";
import { t } from "/assets/js/i18n.js";
import { toneDot, toneSection, worstTone } from "/assets/js/utils/verdict.js";

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

function collectInferenceModels(snap) {
  const seen = new Set();
  const out = [];
  for (const host of (snap.host_apps || [])) {
    for (const m of ((host.health && host.health.inference_models) || [])) {
      if (!seen.has(m)) { seen.add(m); out.push(m); }
    }
  }
  return out;
}

export class SpProxyStatus extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
  }

  onConnect() {
    this.useSnapshot((s) => { this.snapshot = s; });
  }

  render() {
    const snap = this.snapshot || {};
    const proxy = snap.local_proxy || {};
    const view = proxyView(proxy);
    const url = proxy.url || "";
    const models = collectInferenceModels(snap);
    const epState = models.length === 0 ? "warn" : "ok";
    const epDot = models.length === 0 ? "sp-dot--warn" : "sp-dot--ok";

    const healthDetails = [
      ["url", escapeHtml(url || "(none)")],
      view.reason ? ["error", escapeHtml(view.reason)] : null,
      proxy.latency_ms != null ? ["latency", `${proxy.latency_ms} ms`] : null,
      ["state", escapeHtml(view.label)],
    ].filter(Boolean);

    const chips = models.length === 0
      ? `<p class="sp-kpi-card__label" data-l10n-id="status-proxy-endpoints-empty">No models configured yet — start an agent to populate.</p>`
      : `<div class="sp-chip-list sp-kpi-card__chips">${models.map((m) => `<span class="sp-chip">${escapeHtml(m)}</span>`).join("")}</div>`;

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
          </div>
        </article>

        <article class="sp-kpi-card" data-state="${epState}">
          <div class="sp-kpi-card__head">
            <span data-l10n-id="status-proxy-endpoints">Inference endpoints</span>
            <span class="sp-dot ${epDot}" aria-hidden="true"></span>
          </div>
          <div class="sp-kpi-card__value">
            <span>${models.length}</span>
            <span class="sp-kpi-card__unit">${models.length === 1 ? "model" : "models"}</span>
          </div>
          ${chips}
          <div class="sp-kpi-card__foot">
            <span class="sp-kpi-card__foot-meta" data-l10n-id="status-proxy-endpoints-detail">Models the proxy advertises to agents.</span>
          </div>
        </article>
      </div>
    `;
  }

  afterRender() {
    const snap = this.snapshot || {};
    const proxy = snap.local_proxy || {};
    const view = proxyView(proxy);
    const models = collectInferenceModels(snap);
    const epState = models.length === 0 ? "warn" : "ok";
    const overall = worstTone(view.tone, epState);
    publishSectionState(this, overall, toneSection(overall));
  }
}

reactive(SpProxyStatus.prototype, ["snapshot"]);
customElements.define("sp-proxy-status", SpProxyStatus);
