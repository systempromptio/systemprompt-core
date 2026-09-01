import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { runAction } from "/assets/js/utils/action.js";
import { t } from "/assets/js/i18n.js";
import { setTheme, setContrast } from "/assets/js/theme.js";
import { gatewayUrlError, loadPrefs, setPref } from "/assets/js/utils/settings-prefs.js";
import { renderSettingsPaths, renderSettingsAppearance, renderSettingsSecurity } from "/assets/js/components/settings-sections.js";
import { renderSettingsPrefsRow } from "/assets/js/components/settings-prefs-row.js";

export class SpSettings extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.prefs = null;
    this.gatewayDraft = null;
    this.registerAction("toggle-autostart", (el) => setPref(this, "autostart", !!el.checked));
    this.registerAction("toggle-auto-update", (el) => setPref(this, "update_automatic", !!el.checked));
    this.registerAction("toggle-session", (el) => setPref(this, "session_enabled", !!el.checked));
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
    this._registerGatewayActions();
  }

  // Why: this used to add `is-setup-mode`, dropping the user into the
  // full-screen first-run wizard to change one field. The wizard has no
  // cancel, so anyone who clicked it to check their URL was stuck there.
  _registerGatewayActions() {
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
        loadPrefs(this);
      },
      success: t("toast-gateway-saved") || "Gateway saved.",
      context: t("settings-gateway-label") || "Gateway URL",
    }));
  }

  onConnect() {
    this.useSnapshot((s) => { this.snapshot = s; });
    loadPrefs(this);
  }

  render() {
    const malformed = (this.prefs || {}).config_malformed;
    const banner = malformed
      ? `<p class="sp-settings__banner" role="alert">${escapeHtml(
          t("settings-config-malformed", { malformed })
            || `Your configuration file could not be read, so nothing can be saved until it is fixed: ${malformed}`)}</p>`
      : "";
    return `
      ${banner}
      ${renderSettingsPaths(this)}
      ${renderSettingsAppearance()}
      ${renderSettingsPrefsRow(this)}
      ${renderSettingsSecurity(this)}
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
