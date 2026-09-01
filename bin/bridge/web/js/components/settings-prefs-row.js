import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";

function startupLabelFor(platform) {
  return platform
    ? `${t("settings-startup-label") || "Start with"} ${platform}`
    : (t("settings-startup-label-generic") || "Start at login");
}

export function renderSettingsPrefsRow(component) {
  const prefs = component.prefs || {};
  const platform = document.body.dataset.platformDisplay || "";
  // The scheduler can decline to answer, which is neither on nor off. An
  // unchecked box that silently refuses to tick is the worse of the two lies.
  const autostart = prefs.autostart || {};
  const autostartUnknown = (autostart.verdict || {}).tone === "unknown";
  const unknownHint = t("settings-startup-unknown") || "could not be determined on this system";
  const startupLabel = startupLabelFor(platform);
  return `
    <div class="sp-row sp-settings__prefs">
      <label class="sp-settings__pref${autostartUnknown ? " is-unavailable" : ""}"
             ${autostartUnknown ? `title="${escapeHtml(unknownHint)}"` : ""}>
        <input type="checkbox" data-action="toggle-autostart"
               ${autostart.installed ? "checked" : ""} ${autostartUnknown ? "disabled" : ""}>
        <span>${escapeHtml(autostartUnknown ? `${startupLabel} — ${unknownHint}` : startupLabel)}</span>
      </label>
      <label class="sp-settings__pref">
        <input type="checkbox" data-action="toggle-auto-update" ${prefs.update_automatic ? "checked" : ""}>
        <span data-l10n-id="settings-auto-update-label">Install updates automatically</span>
      </label>
      <label class="sp-settings__pref">
        <input type="checkbox" data-action="toggle-session" ${prefs.session_enabled ? "checked" : ""}>
        <span data-l10n-id="settings-session-label">Sign in through the browser instead of a personal access token</span>
      </label>
    </div>`;
}
