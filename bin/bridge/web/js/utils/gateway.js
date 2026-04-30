import { t } from "/assets/js/i18n.js";

export function probeView(snap) {
  const status = (snap && snap.gateway_status) || { state: "unknown" };
  if (status.state === "reachable") {
    return { dot: "sp-dot--ok", muted: false, text: t("setup-gateway-reachable", { latency: status.latency_ms }) || `reachable · ${status.latency_ms}ms` };
  }
  if (status.state === "probing") {
    return { dot: "sp-dot--probing", muted: true, text: t("setup-gateway-probing") || "probing…" };
  }
  if (status.state === "unreachable") {
    return { dot: "sp-dot--err", muted: false, text: t("setup-gateway-unreachable", { reason: status.reason || "unknown" }) || `unreachable · ${status.reason || "unknown"}` };
  }
  const empty = !(snap && snap.gateway_url);
  return { dot: "sp-dot--unknown", muted: true, text: empty ? (t("setup-gateway-empty") || "enter a URL to probe…") : (t("setup-gateway-not-probed") || "not probed yet") };
}

export function probeErrorMessage(snap) {
  if (!snap) { return ""; }
  const status = snap.gateway_status || { state: "unknown" };
  const verified = snap.verified_identity && snap.verified_identity.user_id;
  if (status.state === "reachable" && snap.pat_present && !verified) {
    return "Token rejected by gateway. Issue a fresh PAT and try again.";
  }
  if (status.state === "unreachable" && snap.pat_present) {
    return `Gateway unreachable: ${status.reason || "unknown error"}`;
  }
  return "";
}

export function isPendingResolved(snap, pendingSinceMs) {
  if (!snap) { return false; }
  const probeState = (snap.gateway_status && snap.gateway_status.state) || "unknown";
  const configured = probeState === "reachable" && snap.verified_identity && snap.verified_identity.user_id;
  const elapsed = pendingSinceMs > 0 ? (Date.now() - pendingSinceMs) : 0;
  return configured || probeState === "unreachable" || elapsed > 15000;
}

export function patLinkFor(gateway) {
  const gw = (gateway || "").trim().replace(/\/+$/, "");
  if (gw) { return `${gw}/admin/login`; }
  return "#";
}

function escapeHtml(s) {
  if (s == null) { return ""; }
  return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
}

export function renderGatewayForm(state) {
  const probe = probeView(state.snapshot);
  const link = patLinkFor(state.gateway);
  const linkDisabled = link === "#";
  const editBtn = state.patSaved ? `<button class="sp-btn-ghost" type="button" data-action="edit-pat">Edit</button>` : "";
  const errBlock = state.error ? `<span class="sp-setup__error">${escapeHtml(state.error)}</span>` : "";
  const btnLabel = state.pending ? (t("setup-connecting") || "Connecting…") : "Connect";
  return `
    <div class="sp-setup__field">
      <label for="setup-gateway" data-l10n-id="setup-gateway-label">Gateway URL</label>
      <input id="setup-gateway" type="url" placeholder="http://127.0.0.1:8080" autocomplete="off" spellcheck="false" data-input="gateway" />
      <div class="sp-setup__status">
        <span class="sp-dot ${probe.dot}" aria-hidden="true"></span>
        <span class="${probe.muted ? "sp-u-muted" : ""}">${escapeHtml(probe.text)}</span>
      </div>
    </div>
    <div class="sp-setup__field">
      <label for="setup-pat" data-l10n-id="setup-pat-label">Personal access token</label>
      <input id="setup-pat" type="password" placeholder="sp-live-…" autocomplete="off" spellcheck="false" data-input="pat" />
      <p class="sp-setup__hint">
        <span data-l10n-id="setup-pat-hint">Don't have one yet?</span>
        <a class="sp-setup__pat-link ${linkDisabled ? "is-disabled" : ""}" href="${escapeHtml(link)}" target="_blank" rel="noopener noreferrer" aria-disabled="${linkDisabled}">Open the gateway admin login →</a>
        ${editBtn}
      </p>
    </div>
    <div class="sp-setup__actions">
      <button class="sp-btn-primary" type="button" ${state.pending ? "disabled" : ""} data-action="connect">
        <span class="sp-btn__label">${escapeHtml(btnLabel)}</span>
      </button>
      ${errBlock}
    </div>
  `;
}
