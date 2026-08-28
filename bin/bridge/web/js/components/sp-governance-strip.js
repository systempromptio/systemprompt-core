import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { fmtRelative } from "/assets/js/utils/format.js";

const IDLE_AFTER_SECONDS = 2 * 60 * 60;

function fmtCount(n) {
  const v = Number(n) || 0;
  if (v >= 1_000_000) { return `${(v / 1_000_000).toFixed(1)}M`; }
  if (v >= 1_000) { return `${(v / 1_000).toFixed(1)}k`; }
  return String(v);
}

function fmtIdleSpan(seconds) {
  if (seconds >= 86400) { return `${Math.floor(seconds / 86400)}d`; }
  if (seconds >= 3600) { return `${Math.floor(seconds / 3600)}h`; }
  return `${Math.max(1, Math.floor(seconds / 60))}m`;
}

// The proxy is the thing that governs. If it is not listening, nothing downstream
// is being governed at all, and that outranks any counter we could show.
function proxyIsDown(snapshot) {
  const state = ((snapshot && snapshot.local_proxy && snapshot.local_proxy.state) || "Unknown").toString();
  return state === "Refused" || state === "Timeout" || state === "HttpError";
}

/**
 * @returns {{state: "ok"|"idle"|"down", headline: string, detail: string[]}}
 */
export function governanceView(snapshot, stats) {
  if (proxyIsDown(snapshot)) {
    return {
      state: "down",
      headline: t("governance-proxy-down") || "Proxy not responding — agents are not being governed",
      detail: [],
    };
  }

  const forwarded = Number(stats.forwarded_total) || 0;
  const lastAt = Number(stats.last_forwarded_at_unix) || 0;
  const age = lastAt ? Math.max(0, Math.floor(Date.now() / 1000) - lastAt) : Infinity;

  if (!forwarded || age > IDLE_AFTER_SECONDS) {
    const span = Number.isFinite(age) ? fmtIdleSpan(age) : fmtIdleSpan(IDLE_AFTER_SECONDS);
    return {
      state: "idle",
      headline: t("governance-idle", { duration: span }) || `No traffic in the last ${span}`,
      detail: [],
    };
  }

  const tokens = (Number(stats.tokens_in_total) || 0) + (Number(stats.tokens_out_total) || 0);
  const detail = [
    t("governance-forwarded", { count: String(forwarded) }) || `${forwarded} forwarded`,
    t("governance-last-request", { ago: fmtRelative(lastAt) }) || `last ${fmtRelative(lastAt)}`,
  ];
  // Both counters start at zero and are only ever stored after a real forward, so
  // zero means "not measured yet". Rendering it as `0ms` would report a
  // measurement the proxy has never taken.
  if (stats.last_status) { detail.push(String(stats.last_status)); }
  if (stats.last_latency_ms) { detail.push(`${stats.last_latency_ms}ms`); }
  if (tokens > 0) {
    detail.push(t("governance-tokens", { tokens: fmtCount(tokens) }) || `${fmtCount(tokens)} tokens`);
  }
  return { state: "ok", headline: t("governance-governing") || "Governing", detail };
}

export class SpGovernanceStrip extends SpElement {
  static get observedAttributes() { return ["variant"]; }

  constructor() {
    super();
    this.snapshot = null;
    this.stats = {};
  }

  attributeChangedCallback() { this.invalidate(); }

  onConnect() {
    bridge.stateSnapshot().then((s) => {
      this.snapshot = s;
      this.stats = (s && s.proxy_stats) || {};
    }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this.bridgeSubscribe("proxy.stats", (stats) => { this.stats = stats || {}; });
    this.bridgeSubscribe("proxy.changed", () => {
      bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    });
  }

  render() {
    const view = governanceView(this.snapshot, this.stats || {});
    const compact = this.getAttribute("variant") === "compact";
    const detail = view.detail.length
      ? `<span class="sp-gov__detail">${view.detail.map((d) => escapeHtml(d)).join('<span class="sp-gov__sep" aria-hidden="true">·</span>')}</span>`
      : "";
    return `
      <div class="sp-gov__inner${compact ? " sp-gov__inner--compact" : ""}">
        <span class="sp-dot sp-dot--${view.state === "ok" ? "ok" : view.state === "idle" ? "warn" : "err"}" aria-hidden="true"></span>
        <span class="sp-gov__headline">${escapeHtml(view.headline)}</span>
        ${detail}
      </div>
    `;
  }

  afterRender() {
    const view = governanceView(this.snapshot, this.stats || {});
    this.setAttribute("data-state", view.state);
    this.setAttribute("role", "status");
  }
}

reactive(SpGovernanceStrip.prototype, ["snapshot", "stats"]);
customElements.define("sp-governance-strip", SpGovernanceStrip);
