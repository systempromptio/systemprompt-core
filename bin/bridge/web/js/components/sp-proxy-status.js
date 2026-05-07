import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { publishSectionState } from "/assets/js/utils/format.js";

function proxyView(proxy) {
  const state = (proxy.state || "Unknown").toString();
  if (state === "Listening") {
    return { state: "ok", dot: "sp-dot--ok", value: String(proxy.latency_ms ?? "?"), unit: "ms", label: "listening" };
  }
  if (state === "Refused")      { return { state: "err",     dot: "sp-dot--err",     value: "—", unit: "", label: "connection refused" }; }
  if (state === "Timeout")      { return { state: "err",     dot: "sp-dot--err",     value: "—", unit: "", label: "timed out" }; }
  if (state === "HttpError")    { return { state: "err",     dot: "sp-dot--err",     value: "—", unit: "", label: "http error", reason: proxy.error || "unknown" }; }
  if (state === "Unconfigured") { return { state: "warn",    dot: "sp-dot--warn",    value: "—", unit: "", label: "awaiting first host-app probe" }; }
  return { state: "unknown", dot: "sp-dot--unknown", value: "—", unit: "", label: "checking…" };
}

function collectInferenceModels(snap) {
  const seen = new Set();
  const out = [];
  for (const host of (snap.host_apps || [])) {
    const raw = host.snapshot && host.snapshot.profile_keys && host.snapshot.profile_keys.inferenceModels;
    if (raw) {
      for (const m of raw.split(",")) {
        const trimmed = m.trim();
        if (trimmed && !seen.has(trimmed)) { seen.add(trimmed); out.push(trimmed); }
      }
    }
  }
  return out;
}

function rollUp(a, b) {
  const order = { err: 4, warn: 3, probing: 2, unknown: 1, ok: 0 };
  return (order[a] >= order[b]) ? a : b;
}

function sectionLabel(state) {
  if (state === "ok") { return "healthy"; }
  if (state === "warn") { return "attention"; }
  if (state === "err") { return "down"; }
  if (state === "probing") { return "checking…"; }
  return "unknown";
}

export class SpProxyStatus extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this.bridgeSubscribe("proxy.changed", () => {
      bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    });
  }

  render() {
    const snap = this.snapshot || {};
    const proxy = snap.local_proxy || { state: "Unknown" };
    const view = proxyView(proxy);
    const url = proxy.url || "";
    const models = collectInferenceModels(snap);
    const epState = models.length === 0 ? "warn" : "ok";
    const epDot = models.length === 0 ? "sp-dot--warn" : "sp-dot--ok";

    const healthDetails = [
      ["url", escapeHtml(url || "(none)")],
      view.reason ? ["error", escapeHtml(view.reason)] : null,
      proxy.latency_ms != null ? ["latency", `${proxy.latency_ms} ms`] : null,
      ["state", escapeHtml(proxy.state || "Unknown")],
    ].filter(Boolean);

    const chips = models.length === 0
      ? `<p class="sp-kpi-card__label" data-l10n-id="status-proxy-endpoints-empty">No models configured yet — start a host app to populate.</p>`
      : `<div class="sp-chip-list sp-kpi-card__chips">${models.map((m) => `<span class="sp-chip">${escapeHtml(m)}</span>`).join("")}</div>`;

    return `
      <div class="sp-kpi-grid">
        <article class="sp-kpi-card" data-state="${view.state}">
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
            <span class="sp-kpi-card__foot-meta" data-l10n-id="status-proxy-endpoints-detail">Models the proxy advertises to host apps.</span>
          </div>
        </article>
      </div>
    `;
  }

  afterRender() {
    const snap = this.snapshot || {};
    const proxy = snap.local_proxy || { state: "Unknown" };
    const view = proxyView(proxy);
    const models = collectInferenceModels(snap);
    const epState = models.length === 0 ? "warn" : "ok";
    const overall = rollUp(view.state, epState);
    publishSectionState(this, overall, sectionLabel(overall));
  }
}

reactive(SpProxyStatus.prototype, ["snapshot"]);
customElements.define("sp-proxy-status", SpProxyStatus);
