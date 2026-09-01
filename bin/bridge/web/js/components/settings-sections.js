import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { themePreference, contrastPreference, forcedDark } from "/assets/js/theme.js";
import { gatewayUrlError } from "/assets/js/utils/settings-prefs.js";

function gatewayReadRow(current) {
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

function gatewayEditRow(draft) {
  const error = gatewayUrlError(draft.trim());
  return `
    <div class="sp-kv">
      <label for="settings-gateway" data-l10n-id="settings-gateway-label">Gateway URL</label>
      <div class="sp-settings__inline">
        <input id="settings-gateway" class="sp-input sp-u-mono" type="url" spellcheck="false"
          value="${escapeHtml(draft)}" data-input="gateway"
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

function scheduleValue(prefs) {
  const schedule = prefs.schedule || {};
  const verdict = schedule.verdict || { tone: "unknown", code: "unknown" };
  return t(`settings-schedule-${verdict.code}`, { label: schedule.label || "" }) || "";
}

export function renderSettingsPaths(component) {
  const prefs = component.prefs || {};
  const snap = component.snapshot || {};
  const current = prefs.gateway_url || snap.gateway_url || "";
  const plugins = snap.plugins_dir || "—";
  const config = snap.config_file || "—";
  const muted = (v) => v === "—" ? "sp-u-muted" : "";
  return `
    <div class="sp-kv__grid">
      ${component.gatewayDraft === null ? gatewayReadRow(current) : gatewayEditRow(component.gatewayDraft)}
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
        <div class="sp-value">${escapeHtml(scheduleValue(prefs))}</div>
      </div>
    </div>`;
}

function selectOptions(options, selected) {
  return options
    .map(([v, text]) => `<option value="${v}" ${selected === v ? "selected" : ""}>${escapeHtml(text)}</option>`)
    .join("");
}

export function renderSettingsAppearance() {
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
  const themeRow = forcedDark() ? "" : `
      <div class="sp-kv">
        <label for="settings-theme" data-l10n-id="settings-theme-label">Appearance</label>
        <select id="settings-theme" class="sp-select" data-change="theme">
          ${selectOptions(themeOptions, themePreference())}
        </select>
      </div>`;
  return `
    <div class="sp-kv__grid">
      ${themeRow}
      <div class="sp-kv">
        <label for="settings-contrast" data-l10n-id="settings-contrast-label">Contrast</label>
        <select id="settings-contrast" class="sp-select" data-change="contrast">
          ${selectOptions(contrastOptions, contrastPreference())}
        </select>
      </div>
    </div>`;
}

/**
 * Read-only, but visible. A supply-chain pin and an mTLS reference the user
 * cannot see are controls they cannot audit — and a managed policy can
 * replace the pin silently, so the provenance is the point of the row.
 */
export function renderSettingsSecurity(component) {
  const prefs = component.prefs || {};
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
