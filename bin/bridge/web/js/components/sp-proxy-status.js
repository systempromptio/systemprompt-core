import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";

function proxyView(proxy) {
  const state = (proxy.state || "Unknown").toString();
  if (state === "Listening") {
    return { dot: "sp-dot--ok", label: `listening · ${proxy.latency_ms ?? "?"}ms` };
  }
  if (state === "Refused") { return { dot: "sp-dot--err", label: "connection refused" }; }
  if (state === "Timeout") { return { dot: "sp-dot--err", label: "timed out" }; }
  if (state === "HttpError") { return { dot: "sp-dot--err", label: `error: ${proxy.error || "unknown"}` }; }
  if (state === "Unconfigured") { return { dot: "sp-dot--warn", label: "awaiting first host-app probe" }; }
  return { dot: "sp-dot--unknown", label: "checking…" };
}

function collectInferenceModels(snap) {
  const seen = new Set();
  const out = [];
  for (const host of (snap.host_apps || [])) {
    const raw = host.snapshot && host.snapshot.profile_keys && host.snapshot.profile_keys.inferenceModels;
    if (raw) {
      for (const m of raw.split(",")) {
        const t = m.trim();
        if (t && !seen.has(t)) { seen.add(t); out.push(t); }
      }
    }
  }
  return out;
}

export class SpProxyStatus extends BridgeElement {
  static properties = { snapshot: { state: true } };

  constructor() {
    super();
    this.snapshot = null;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this.bridgeSubscribe("proxy.changed", () => {
      bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
    });
  }

  render() {
    const snap = this.snapshot || {};
    const proxy = snap.local_proxy || { state: "Unknown" };
    const view = proxyView(proxy);
    const url = proxy.url;
    const models = collectInferenceModels(snap);
    const epDot = models.length === 0 ? "sp-dot--unknown" : "sp-dot--ok";
    const epText = models.length === 0 ? "no models configured yet" : models.join(", ");
    const epMuted = models.length === 0;

    return html`
      <table class="sp-status__board">
        <tbody>
          <tr>
            <th data-l10n-id="status-proxy-health">Health</th>
            <td>
              <div class="sp-status__row">
                <span class="sp-dot ${view.dot}" aria-hidden="true"></span>
                <span>${view.label}</span>
              </div>
              <div class="sp-status__detail sp-u-mono ${url ? "" : "sp-u-muted"}">${url || "(no proxy URL configured yet)"}</div>
            </td>
            <td class="sp-status__actions"></td>
          </tr>
          <tr>
            <th data-l10n-id="status-proxy-endpoints">Inference endpoints</th>
            <td>
              <div class="sp-status__row">
                <span class="sp-dot ${epDot}" aria-hidden="true"></span>
                <span class=${epMuted ? "sp-u-muted" : ""}>${epText}</span>
              </div>
              <div class="sp-status__detail sp-u-muted" data-l10n-id="status-proxy-endpoints-detail">Models the proxy advertises to host apps.</div>
            </td>
            <td class="sp-status__actions"></td>
          </tr>
        </tbody>
      </table>
    `;
  }
}

customElements.define("sp-proxy-status", SpProxyStatus);
