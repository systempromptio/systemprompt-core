import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { fmtDuration } from "/assets/js/utils/format.js";

const TOKEN_WARN_SECONDS = 600;

function cloudPill(snap) {
  const status = snap.gateway_status || { state: "unknown" };
  if (status.state !== "reachable") {
    return { state: "err", text: t("agents-status-cloud-unreachable") || "cloud unreachable" };
  }
  const id = snap.verified_identity;
  if (id && (id.email || id.user_id)) {
    const who = id.email || id.user_id;
    return {
      state: "ok",
      text: t("agents-status-cloud-signed-in", { email: who }) || `signed in as ${who}`,
    };
  }
  return { state: "warn", text: t("agents-status-cloud-signed-out") || "signed out" };
}

function proxyPill(snap) {
  const proxy = snap.local_proxy || { state: "Unknown" };
  const state = String(proxy.state || "Unknown");
  if (state === "Listening") {
    const latency = proxy.latency_ms != null ? String(proxy.latency_ms) : "?";
    const httpStatus = proxy.http_status != null ? String(proxy.http_status) : "—";
    return {
      state: "ok",
      text: t("agents-status-proxy-listening", { latency, status: httpStatus })
        || `Listening · ${latency}ms · ${httpStatus}`,
    };
  }
  if (state === "Refused") {
    return { state: "err", text: t("agents-status-proxy-refused") || "proxy refused" };
  }
  if (state === "Timeout") {
    return { state: "err", text: t("agents-status-proxy-timeout") || "proxy timed out" };
  }
  if (state === "HttpError") {
    return { state: "err", text: t("agents-status-proxy-http-error") || "proxy http error" };
  }
  if (state === "Unconfigured") {
    return { state: "warn", text: t("agents-status-proxy-unconfigured") || "proxy unconfigured" };
  }
  return { state: "warn", text: t("agents-status-proxy-unconfigured") || "proxy unconfigured" };
}

function tokenPill(snap) {
  const token = snap.cached_token;
  if (!token) {
    return { state: "err", text: t("agents-status-token-missing") || "no token" };
  }
  const ttl = Number(token.ttl_seconds);
  const ttlText = fmtDuration(ttl);
  if (ttl < TOKEN_WARN_SECONDS) {
    return {
      state: "warn",
      text: t("agents-status-token-expiring", { ttl: ttlText }) || `JWT expires in ${ttlText}`,
    };
  }
  return {
    state: "ok",
    text: t("agents-status-token-ok", { ttl: ttlText }) || `JWT ok · expires in ${ttlText}`,
  };
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
    bridge.stateSnapshot()
      .then((s) => { this.snapshot = s; })
      .catch((e) => console.warn("sp-agents-status snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
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
        class="sp-agents-status__pill sp-agents-status__pill--${view.state}"
        data-action="goto-status-${key}"
        title="${escapeHtml(view.text)}"
      >${escapeHtml(view.text)}</button>
    `;
    return `
      <div class="sp-agents-status" role="group" aria-label="Bridge status">
        ${pill("cloud", cloud)}
        ${pill("proxy", proxy)}
        ${pill("token", token)}
      </div>
    `;
  }
}

reactive(SpAgentsStatus.prototype, ["snapshot"]);
customElements.define("sp-agents-status", SpAgentsStatus);
