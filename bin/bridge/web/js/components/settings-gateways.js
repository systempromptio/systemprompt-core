import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";
import { bridge } from "/assets/js/bridge.js";
import { runAction } from "/assets/js/utils/action.js";
import { gatewayUrlError } from "/assets/js/utils/settings-prefs.js";

export function registerGatewayProfileActions(component) {
  component.profileDraft = null;
  component.registerAction("add-profile", () => { component.profileDraft = ""; });
  component.registerAction("cancel-profile", () => { component.profileDraft = null; });
  component.registerAction("input:profile", (el) => { component.profileDraft = el.value; });
  component.registerAction("save-profile", (trigger) => runAction(trigger, {
    run: async () => {
      const url = (component.profileDraft || "").trim();
      const invalid = gatewayUrlError(url);
      if (invalid) { throw new Error(invalid); }
      await bridge.gatewayAdd(url);
      component.profileDraft = null;
    },
    success: t("toast-gateway-profile-added") || "Gateway added.",
    context: t("settings-gateways-heading") || "Gateways",
  }));
  component.registerAction("use-profile", (trigger) => runAction(trigger, {
    run: () => bridge.gatewayUse(trigger.dataset.profileId),
    success: t("toast-gateway-profile-switched") || "Switched gateway; syncing now.",
    context: t("settings-gateways-heading") || "Gateways",
  }));
  component.registerAction("remove-profile", (trigger) => runAction(trigger, {
    run: () => bridge.gatewayRemove(trigger.dataset.profileId),
    success: t("toast-gateway-profile-removed") || "Gateway removed.",
    context: t("settings-gateways-heading") || "Gateways",
  }));
  component.registerAction("disable-instance", (trigger) => runAction(trigger, {
    run: () => bridge.instanceDisable(),
    success: t("toast-instance-disabled") || "Disabled. Your agents use their own sign-in.",
    context: t("settings-instance-label") || "This device",
  }));
  component.registerAction("enable-instance", (trigger) => runAction(trigger, {
    run: () => bridge.instanceEnable(),
    success: t("toast-instance-enabled") || "Enabled; syncing now.",
    context: t("settings-instance-label") || "This device",
  }));
}

function profileRow(profile) {
  const id = escapeHtml(profile.id);
  const name = profile.display_name ? escapeHtml(profile.display_name) : id;
  const url = escapeHtml(profile.gateway_url || "—");
  const badge = profile.active
    ? `<span class="sp-badge sp-badge--ok" data-l10n-id="settings-gateways-active">Active</span>`
    : "";
  const disabled = profile.disabled
    ? `<span class="sp-badge sp-badge--warn" data-l10n-id="settings-gateways-disabled">Disabled</span>`
    : "";
  const actions = profile.active ? "" : `
        <button class="sp-btn-ghost" type="button" data-action="use-profile" data-profile-id="${id}"
          data-l10n-id="settings-gateways-use">Switch</button>
        <button class="sp-btn-ghost" type="button" data-action="remove-profile" data-profile-id="${id}"
          data-l10n-id="settings-gateways-remove">Remove</button>`;
  return `
    <li class="sp-settings__profile">
      <div class="sp-settings__inline">
        <div class="sp-value">${name} ${badge} ${disabled}</div>
        <div class="sp-value sp-u-mono sp-u-muted">${url}</div>
        ${actions}
      </div>
    </li>`;
}

function addRow(draft) {
  if (draft === null) {
    return `<button class="sp-btn-ghost" type="button" data-action="add-profile"
      data-l10n-id="settings-gateways-add">Add gateway</button>`;
  }
  const error = draft.trim() ? gatewayUrlError(draft.trim()) : "";
  return `
    <div class="sp-settings__inline">
      <input id="settings-profile-url" class="sp-input sp-u-mono" type="url" spellcheck="false"
        value="${escapeHtml(draft)}" data-input="profile" aria-invalid="${error ? "true" : "false"}">
      <button class="sp-btn" type="button" data-action="save-profile" ${error || !draft.trim() ? "disabled" : ""}
        data-l10n-id="settings-gateway-save">Save</button>
      <button class="sp-btn-ghost" type="button" data-action="cancel-profile"
        data-l10n-id="settings-gateway-cancel">Cancel</button>
    </div>
    ${error ? `<p class="sp-settings__error" role="alert">${escapeHtml(error)}</p>` : ""}`;
}

function windowNotice(win) {
  if (!win || win.tone === "ok" || win.tone === "unknown") return "";
  const range = `[${win.min || "any"}, ${win.max || "any"}]`;
  let text;
  if (win.conflict) {
    text = t("settings-window-conflict", { conflict: win.conflict })
      || `Your gateways admit no common bridge release (${win.conflict}); no update is installed automatically.`;
  } else if (win.status === "below") {
    text = t("settings-window-below", { range })
      || `This bridge release is older than your gateway admits ${range}; the admitted release is being installed.`;
  } else {
    text = t("settings-window-above", { range })
      || `This bridge release is newer than your gateway admits ${range}; install the offered release from Updates.`;
  }
  return `<p class="sp-settings__banner" role="status" data-tone="${escapeHtml(win.tone)}">${escapeHtml(text)}</p>`;
}

function instanceRow(snap) {
  if (snap.instance_disabled) {
    return `
      <div class="sp-kv">
        <label data-l10n-id="settings-instance-label">This device</label>
        <div class="sp-settings__inline">
          <div class="sp-value" data-l10n-id="settings-instance-disabled">Disabled — agents use their own sign-in</div>
          <button class="sp-btn" type="button" data-action="enable-instance"
            data-l10n-id="settings-instance-enable">Enable</button>
        </div>
      </div>`;
  }
  if (!snap.disable_allowed) return "";
  return `
    <div class="sp-kv">
      <label data-l10n-id="settings-instance-label">This device</label>
      <div class="sp-settings__inline">
        <div class="sp-value sp-u-muted" data-l10n-id="settings-instance-disable-hint">Stop routing agents through the gateway so they use your own subscriptions</div>
        <button class="sp-btn-ghost" type="button" data-action="disable-instance"
          data-l10n-id="settings-instance-disable">Disable</button>
      </div>
    </div>`;
}

export function renderSettingsGateways(component) {
  const snap = component.snapshot || {};
  const profiles = Array.isArray(snap.gateway_profiles) ? snap.gateway_profiles : [];
  return `
    <section class="sp-settings__section">
      <h2 data-l10n-id="settings-gateways-heading">Gateways</h2>
      ${windowNotice(snap.release_window)}
      <ul class="sp-settings__profiles">
        ${profiles.map(profileRow).join("")}
      </ul>
      ${addRow(component.profileDraft === undefined ? null : component.profileDraft)}
      <div class="sp-kv__grid">${instanceRow(snap)}</div>
    </section>`;
}
