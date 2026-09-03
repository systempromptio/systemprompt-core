import { t } from "/assets/js/i18n.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { toneDot } from "/assets/js/utils/verdict.js";

export function probeView(snap) {
  const gateway = (snap && snap.gateway_status) || { tone: "unknown", code: "unknown", settled: false };
  const empty = !(snap && snap.gateway_url);
  if (!gateway.settled && empty) {
    return { dot: toneDot("unknown"), muted: true, text: t("setup-gateway-empty") || "enter a URL to probe…" };
  }
  return {
    dot: toneDot(gateway.tone),
    muted: gateway.tone !== "ok" && gateway.tone !== "err",
    text: t(`gateway-state-${gateway.code}`, { latency: gateway.latency_ms ?? "?", reason: gateway.reason || "unknown" }) || "",
  };
}

// Why: whether an unverified token is *rejected* or merely *unverified yet* is
// the bridge's call (`identity`), not this form's — Setup and Status used to
// answer it differently from the same snapshot.
export function probeErrorMessage(snap) {
  if (!snap) { return ""; }
  const identity = snap.identity || {};
  const gateway = snap.gateway_status || {};
  if (identity.code === "token-rejected") {
    return t("setup-token-rejected")
      || "The gateway rejected that personal access token. Issue a fresh one and try again.";
  }
  if (identity.code === "gateway-unreachable" && snap.pat_present) {
    const reason = gateway.reason || "unknown error";
    return t("setup-gateway-unreachable-reason", { reason }) || `Gateway unreachable: ${reason}`;
  }
  return "";
}

export function isPendingResolved(snap, pendingSinceMs) {
  if (!snap) { return false; }
  const gateway = snap.gateway_status || {};
  const elapsed = pendingSinceMs > 0 ? (Date.now() - pendingSinceMs) : 0;
  return !!snap.signed_in || gateway.tone === "err" || elapsed > 15000;
}

export function patLinkFor(gateway) {
  const gw = (gateway || "").trim().replace(/\/+$/, "");
  if (gw) { return `${gw}/admin/login`; }
  return "#";
}

const SPINNER = `<span class="sp-btn__spinner" aria-hidden="true"></span>`;

export function renderGatewayForm(state) {
  const probe = probeView(state.snapshot);
  const link = patLinkFor(state.gateway);
  const linkDisabled = link === "#";
  const editBtn = state.patSaved ? `<button class="sp-btn-ghost" type="button" data-action="edit-pat" data-l10n-id="setup-pat-edit">Edit</button>` : "";
  const errBlock = state.error ? `<span class="sp-setup__error">${escapeHtml(state.error)}</span>` : "";
  const btnLabel = state.pending ? (t("setup-connecting") || "Connecting…") : (t("setup-connect") || "Connect");
  const snap = state.snapshot || {};
  const signInLabel = snap.sign_in_label || t("setup-sign-in-default") || "Sign in to your gateway";
  const signInHint = snap.sign_in_hint || t("setup-sign-in-hint") || "Opens your browser to sign in on the gateway; this device is linked automatically.";
  const signInBusy = state.signingIn;
  const signInText = signInBusy ? (t("setup-signing-in") || "Waiting for your browser…") : signInLabel;
  const keepChecked = state.keepSignedIn === false ? "" : "checked";
  // The device-link flow round-trips through the gateway's browser login, so an
  // unreachable gateway can only ever fail — gate the button and say why rather
  // than opening a browser at a dead host.
  const reachable = (snap.gateway_status || {}).tone === "ok";
  const signInDisabled = signInBusy || state.pending || !reachable;
  const gateReason = reachable || signInBusy || state.pending
    ? ""
    : `<p class="sp-setup__hint sp-setup__hint--gate">${escapeHtml(t("setup-gateway-required") || "Check the gateway URL under Advanced, then try again.")}</p>`;
  const cancelBtn = signInBusy
    ? `<button class="sp-btn-ghost" type="button" data-action="cancel-sign-in">
        <span class="sp-btn__label">${escapeHtml(t("setup-sign-in-cancel") || "Cancel")}</span>
      </button>`
    : "";
  return `
    <div class="sp-setup__field">
      <label for="setup-gateway" data-l10n-id="setup-gateway-label">Gateway URL</label>
      <input id="setup-gateway" type="url" placeholder="http://127.0.0.1:8080" data-l10n-placeholder="setup-gateway-placeholder" autocomplete="off" spellcheck="false" data-input="gateway" />
      <div class="sp-setup__status" role="status" aria-live="polite">
        <span class="sp-dot ${probe.dot}" aria-hidden="true"></span>
        <span class="${probe.muted ? "sp-u-muted" : ""}">${escapeHtml(probe.text)}</span>
      </div>
    </div>
    <div class="sp-setup__actions">
      <button class="sp-btn-primary ${signInBusy ? "is-busy" : ""}" type="button" ${signInDisabled ? "disabled" : ""} data-action="sign-in">
        ${signInBusy ? SPINNER : ""}<span class="sp-btn__label">${escapeHtml(signInText)}</span>
      </button>
      ${cancelBtn}
      ${gateReason}
      <label class="sp-setup__keep">
        <input id="setup-keep" type="checkbox" ${keepChecked} ${signInBusy ? "disabled" : ""} data-input="keep" />
        <span data-l10n-id="setup-keep-signed-in">Keep me signed in on this device</span>
      </label>
      <p class="sp-setup__hint">${escapeHtml(signInHint)}</p>
    </div>
    <details class="sp-setup__advanced">
      <summary data-l10n-id="setup-pat-summary">Use a personal access token instead</summary>
      <div class="sp-setup__field">
        <label for="setup-pat" data-l10n-id="setup-pat-label">Personal access token</label>
        <input id="setup-pat" type="password" placeholder="sp-live-…" data-l10n-placeholder="setup-pat-placeholder" autocomplete="off" spellcheck="false" data-input="pat" />
        <p class="sp-setup__hint">
          <span data-l10n-id="setup-pat-hint">Don't have one yet?</span>
          <a class="sp-setup__pat-link ${linkDisabled ? "is-disabled" : ""}" href="${escapeHtml(link)}" target="_blank" rel="noopener noreferrer" aria-disabled="${linkDisabled}" data-l10n-id="setup-pat-open-login">Open the gateway login →</a>
          ${editBtn}
        </p>
      </div>
      <div class="sp-setup__actions">
        <button class="sp-btn-ghost ${state.pending ? "is-busy" : ""}" type="button" ${state.pending ? "disabled" : ""} data-action="connect">
          ${state.pending ? SPINNER : ""}<span class="sp-btn__label">${escapeHtml(btnLabel)}</span>
        </button>
      </div>
    </details>
    <sp-setup-purge></sp-setup-purge>
    ${errBlock}
  `;
}
