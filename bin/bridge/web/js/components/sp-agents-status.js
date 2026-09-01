import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { fmtDurationLong } from "/assets/js/utils/format.js";

// Three pills, three verdicts, all computed by the bridge. Each one here is a
// tone plus a catalogue lookup on the code.
function cloudPill(snap) {
  const identity = snap.identity || { tone: "unknown", code: "signed-out" };
  const id = snap.verified_identity || {};
  const email = id.email || id.user_id || "";
  return { tone: identity.tone, text: t(`agents-status-cloud-${identity.code}`, { email }) || "" };
}

function proxyPill(snap) {
  const proxy = snap.local_proxy || {};
  const verdict = proxy.verdict || { tone: "unknown", code: "unknown" };
  const latency = proxy.latency_ms != null ? String(proxy.latency_ms) : "?";
  const status = proxy.http_status != null ? String(proxy.http_status) : "—";
  return { tone: verdict.tone, text: t(`agents-status-proxy-${verdict.code}`, { latency, status }) || "" };
}

function tokenPill(snap) {
  const token = snap.token || { tone: "unknown", code: "missing" };
  const ttl = fmtDurationLong(Number((snap.cached_token || {}).ttl_seconds || 0));
  return { tone: token.tone, text: t(`agents-status-token-${token.code}`, { ttl }) || "" };
}

function gotoStatus() {
  const rail = document.querySelector("sp-rail");
  if (rail && typeof rail.activateTab === "function") {
    rail.activateTab("status");
  }
}

export class SpAgentsStatus extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.registerAction("goto-status-cloud", () => gotoStatus());
    this.registerAction("goto-status-proxy", () => gotoStatus());
    this.registerAction("goto-status-token", () => gotoStatus());
  }

  onConnect() {
    this.useSnapshot((s) => { this.snapshot = s; });
  }

  render() {
    const snap = this.snapshot;
    if (!snap) {
      return `<div class="sp-agents-status sp-agents-status--loading" aria-hidden="true"></div>`;
    }
    const cloud = cloudPill(snap);
    const proxy = proxyPill(snap);
    const token = tokenPill(snap);
    const pill = (key, view) => `
      <button
        type="button"
        class="sp-agents-status__pill sp-agents-status__pill--${view.tone}"
        data-action="goto-status-${key}"
        title="${escapeHtml(view.text)}"
      >${escapeHtml(view.text)}</button>
    `;
    return `
      <div class="sp-agents-status" role="group" data-l10n-aria="agents-status-group-aria" aria-label="Bridge status">
        ${pill("cloud", cloud)}
        ${pill("proxy", proxy)}
        ${pill("token", token)}
      </div>
    `;
  }
}

reactive(SpAgentsStatus.prototype, ["snapshot"]);
customElements.define("sp-agents-status", SpAgentsStatus);
