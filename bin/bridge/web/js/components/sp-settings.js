import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { runAction } from "/assets/js/utils/action.js";
import { notifyErr, notifyOk } from "/assets/js/utils/notify.js";
import { t } from "/assets/js/i18n.js";
import { themePreference, contrastPreference, setTheme, setContrast } from "/assets/js/theme.js";

/**
 * Inline validation for the gateway field. The wizard's own validation lived
 * behind a flow with no exit, so a user checking their URL could not reach it.
 */
export function gatewayUrlError(value) {
  if (!value) { return t("settings-gateway-empty") || "Enter a gateway URL."; }
  let parsed;
  try { parsed = new URL(value); } catch (_) {
    return t("settings-gateway-invalid") || "That is not a valid URL.";
  }
  if (parsed.protocol !== "https:" && parsed.hostname !== "localhost" && parsed.hostname !== "127.0.0.1") {
    return t("settings-gateway-https") || "The gateway must be https, except on localhost.";
  }
  return "";
}

export class SpSettings extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.prefs = null;
    this.gatewayDraft = null;
    this.registerAction("toggle-autostart", (el) => this._setPref("autostart", !!el.checked));
    this.registerAction("toggle-auto-update", (el) => this._setPref("update_automatic", !!el.checked));
    this.registerAction("toggle-session", (el) => this._setPref("session_enabled", !!el.checked));
    this.registerAction("open-folder", (trigger) => runAction(trigger, {
      run: () => bridge.openConfigFolder(),
      success: t("toast-folder-opened") || "Opened the configuration folder.",
      context: t("settings-action-open-folder") || "Open config folder",
    }));
    this.registerAction("validate", (trigger) => runAction(trigger, {
      run: () => bridge.validate(),
      success: t("toast-validate-ok") || "Configuration validated.",
      context: t("settings-action-validate") || "Re-check",
    }));
    this.registerAction("change:theme", (el) => { setTheme(el.value); this.invalidate(); });
    this.registerAction("change:contrast", (el) => { setContrast(el.value); this.invalidate(); });
    // Why: this used to add `is-setup-mode`, dropping the user into the
    // full-screen first-run wizard to change one field. The wizard has no
    // cancel, so anyone who clicked it to check their URL was stuck there.
    this.registerAction("edit-gateway", () => {
      this.gatewayDraft = (this.prefs && this.prefs.gateway_url)
        || (this.snapshot && this.snapshot.gateway_url) || "";
    });
    this.registerAction("cancel-gateway", () => { this.gatewayDraft = null; });
    this.registerAction("input:gateway", (el) => { this.gatewayDraft = el.value; });
    this.registerAction("save-gateway", (trigger) => runAction(trigger, {
      run: async () => {
        const url = (this.gatewayDraft || "").trim();
        const invalid = gatewayUrlError(url);
        if (invalid) { throw new Error(invalid); }
        await bridge.gatewaySet(url);
        this.gatewayDraft = null;
        this._loadPrefs();
      },
      success: t("toast-gateway-saved") || "Gateway saved.",
      context: t("settings-gateway-label") || "Gateway URL",
    }));
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this._loadPrefs();
  }

  _loadPrefs() {
    bridge.settingsGet()
      .then((p) => { this.prefs = p; })
      .catch((e) => console.warn("settings read failed", e));
  }

  // Why: the checkbox is the source of truth for the click that just happened,
  // but the registration it triggers can fail (a locked-down machine refuses
  // schtasks), so the reply — not the DOM — decides what renders next.
  _setPref(key, value) {
    return bridge.settingsSet(key, value)
      .then((p) => { this.prefs = p; notifyOk(t("toast-setting-saved") || "Saved."); })
      .catch((e) => { notifyErr(e, t("settings-prefs-label") || "Startup and updates"); this._loadPrefs(); });
  }

  _gatewayRow() {
    const prefs = this.prefs || {};
    const snap = this.snapshot || {};
    const current = prefs.gateway_url || snap.gateway_url || "";
    if (this.gatewayDraft === null) {
      return `
        <div class="sp-kv">
          <label data-l10n-id="settings-gateway-label">Gateway URL</label>
          <div class="sp-settings__inline">
            <div class="sp-value sp-u-mono ${current ? "" : "sp-u-muted"}">${escapeHtml(current || "—")}</div>
            <button class="sp-btn-ghost" type="button" data-action="edit-gateway"
              data-l10n-id="settings-action-change-gateway">Change</button>
          </div>
        </div>`;
    }
    const error = gatewayUrlError(this.gatewayDraft.trim());
    return `
      <div class="sp-kv">
        <label for="settings-gateway" data-l10n-id="settings-gateway-label">Gateway URL</label>
        <div class="sp-settings__inline">
          <input id="settings-gateway" class="sp-input sp-u-mono" type="url" spellcheck="false"
            value="${escapeHtml(this.gatewayDraft)}" data-input="gateway"
            aria-invalid="${error ? "true" : "false"}"
            ${error ? 'aria-describedby="settings-gateway-error"' : ""}>
          <button class="sp-btn" type="button" data-action="save-gateway" ${error ? "disabled" : ""}
            data-l10n-id="settings-gateway-save">Save</button>
          <button class="sp-btn-ghost" type="button" data-action="cancel-gateway"
            data-l10n-id="settings-gateway-cancel">Cancel</button>
        </div>
        ${error ? `<p id="settings-gateway-error" class="sp-settings__error" role="alert">${escapeHtml(error)}</p>` : ""}
      </div>`;
  }

  /**
   * Read-only, but visible. A supply-chain pin and an mTLS reference the user
   * cannot see are controls they cannot audit — and a managed policy can
   * replace the pin silently, so the provenance is the point of the row.
   */
  _securitySection() {
    const prefs = this.prefs || {};
    const pin = prefs.pinned_pubkey;
    const keystore = prefs.cert_keystore_ref;
    const pinBody = pin
      ? `<div class="sp-value sp-u-mono">${escapeHtml(pin.value)}</div>
         <span class="sp-badge ${pin.source === "policy" ? "sp-badge--warn" : "sp-badge--muted"}">${
           escapeHtml(pin.source === "policy"
             ? (t("settings-pin-source-policy") || "Set by device policy")
             : (t("settings-pin-source-operator") || "Set on this device"))}</span>`
      : `<div class="sp-value sp-u-muted" data-l10n-id="settings-pin-none">Not pinned — the first sync will trust and pin whatever key it is served</div>`;
    return `
      <section class="sp-settings__section">
        <h2 data-l10n-id="settings-security-heading">Security</h2>
        <div class="sp-kv__grid">
          <div class="sp-kv">
            <label data-l10n-id="settings-pin-label">Manifest signing key</label>
            ${pinBody}
          </div>
          <div class="sp-kv">
            <label data-l10n-id="settings-mtls-label">Client certificate</label>
            <div class="sp-value sp-u-mono ${keystore ? "" : "sp-u-muted"}">${escapeHtml(keystore || (t("settings-mtls-none") || "Not configured"))}</div>
          </div>
        </div>
      </section>`;
  }

  _scheduleValue() {
    const schedule = (this.prefs || {}).schedule || {};
    if (schedule.state === "installed") {
      return t("settings-schedule-installed", { label: schedule.label || "" })
        || `Registered with the system scheduler${schedule.label ? ` as ${schedule.label}` : ""}`;
    }
    if (schedule.state === "not_installed") {
      return t("settings-schedule-manual") || "Manual — sync from the Marketplace pane";
    }
    return t("settings-schedule-unknown") || "Could not be determined on this system";
  }

  render() {
    const snap = this.snapshot || {};
    const plugins = snap.plugins_dir || "—";
    const config = snap.config_file || "—";
    const muted = (v) => v === "—" ? "sp-u-muted" : "";
    const prefs = this.prefs || {};
    const platform = document.body.dataset.platformDisplay || "";
    // The scheduler can decline to answer, which is neither on nor off. An
    // unchecked box that silently refuses to tick is the worse of the two lies.
    const autostart = (prefs.autostart || {}).state;
    const autostartUnknown = autostart === "unknown";
    const autostartUnknownHint = t("settings-startup-unknown")
      || "could not be determined on this system";
    const startupLabel = platform
      ? `${t("settings-startup-label") || "Start with"} ${platform}`
      : (t("settings-startup-label-generic") || "Start at login");
    const theme = themePreference();
    const contrast = contrastPreference();
    const themeOptions = [
      ["system", t("settings-theme-system") || "Match my system"],
      ["light", t("settings-theme-light") || "Light"],
      ["dark", t("settings-theme-dark") || "Dark"],
    ];
    const contrastOptions = [
      ["system", t("settings-contrast-system") || "Match my system"],
      ["default", t("settings-contrast-default") || "Standard"],
      ["elevated", t("settings-contrast-elevated") || "Increased"],
    ];
    const malformed = prefs.config_malformed;
    const banner = malformed
      ? `<p class="sp-settings__banner" role="alert">${escapeHtml(
          t("settings-config-malformed", { malformed })
            || `Your configuration file could not be read, so nothing can be saved until it is fixed: ${malformed}`)}</p>`
      : "";
    return `
      ${banner}
      <div class="sp-kv__grid">
        ${this._gatewayRow()}
        <div class="sp-kv">
          <label data-l10n-id="settings-plugins-label">Plugins directory</label>
          <div class="sp-value sp-u-mono ${muted(plugins)}">${escapeHtml(plugins)}</div>
        </div>
        <div class="sp-kv">
          <label data-l10n-id="settings-config-label">Config file</label>
          <div class="sp-value sp-u-mono ${muted(config)}">${escapeHtml(config)}</div>
        </div>
        <div class="sp-kv">
          <label data-l10n-id="settings-schedule-label">Sync schedule</label>
          <div class="sp-value">${escapeHtml(this._scheduleValue())}</div>
        </div>
      </div>
      <div class="sp-kv__grid">
        <div class="sp-kv">
          <label for="settings-theme" data-l10n-id="settings-theme-label">Appearance</label>
          <select id="settings-theme" class="sp-select" data-change="theme">
            ${themeOptions.map(([v, text]) => `<option value="${v}" ${theme === v ? "selected" : ""}>${escapeHtml(text)}</option>`).join("")}
          </select>
        </div>
        <div class="sp-kv">
          <label for="settings-contrast" data-l10n-id="settings-contrast-label">Contrast</label>
          <select id="settings-contrast" class="sp-select" data-change="contrast">
            ${contrastOptions.map(([v, text]) => `<option value="${v}" ${contrast === v ? "selected" : ""}>${escapeHtml(text)}</option>`).join("")}
          </select>
        </div>
      </div>
      <div class="sp-row sp-settings__prefs">
        <label class="sp-settings__pref${autostartUnknown ? " is-unavailable" : ""}"
               ${autostartUnknown ? `title="${escapeHtml(autostartUnknownHint)}"` : ""}>
          <input type="checkbox" data-action="toggle-autostart"
                 ${autostart === "installed" ? "checked" : ""} ${autostartUnknown ? "disabled" : ""}>
          <span>${escapeHtml(autostartUnknown ? `${startupLabel} — ${autostartUnknownHint}` : startupLabel)}</span>
        </label>
        <label class="sp-settings__pref">
          <input type="checkbox" data-action="toggle-auto-update" ${prefs.update_automatic ? "checked" : ""}>
          <span data-l10n-id="settings-auto-update-label">Install updates automatically</span>
        </label>
        <label class="sp-settings__pref">
          <input type="checkbox" data-action="toggle-session" ${prefs.session_enabled ? "checked" : ""}>
          <span data-l10n-id="settings-session-label">Sign in through the browser instead of a personal access token</span>
        </label>
      </div>
      ${this._securitySection()}
      <div class="sp-row">
        <button class="sp-btn-ghost" type="button" data-l10n-id="settings-action-open-folder" data-action="open-folder">Open config folder</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="settings-action-validate" data-action="validate">Run validate</button>
      </div>
      <p class="sp-settings__note sp-u-muted">
        <span data-l10n-id="settings-licensing-note-prefix">Demo build — for production licensing contact</span>
        <a href="mailto:ed@systemprompt.io">ed@systemprompt.io</a>.
      </p>
    `;
  }
}

reactive(SpSettings.prototype, ["snapshot", "prefs", "gatewayDraft"]);
customElements.define("sp-settings", SpSettings);
