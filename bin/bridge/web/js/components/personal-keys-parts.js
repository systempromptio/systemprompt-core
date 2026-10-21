// Markup for the Settings "Personal subscriptions" section. Every failure a row
// can be in renders as a badge, the reason in words, and the one button that
// fixes it — never a silent empty row.

import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";

const MASK = "•".repeat(24);

function fixButton(reason, provider) {
  const target = provider ? ` data-provider="${escapeHtml(provider)}"` : "";
  if (reason === "signed_out") {
    return `<button class="sp-btn" type="button" data-action="pk-sign-in">${escapeHtml(t("personal-keys-fix-sign-in") || "Sign in")}</button>`;
  }
  if (reason === "key_rejected") {
    return `<button class="sp-btn" type="button" data-action="pk-edit"${target}>${escapeHtml(t("personal-keys-fix-reenter") || "Re-enter key")}</button>`;
  }
  if (reason === "not_offered") {
    return `<button class="sp-btn" type="button" data-action="pk-reload">${escapeHtml(t("personal-keys-fix-refresh") || "Refresh list")}</button>`;
  }
  return `<button class="sp-btn" type="button" data-action="pk-retry"${target}>${escapeHtml(t("personal-keys-fix-retry") || "Try again")}</button>`;
}

export function failureBlock(error, provider) {
  if (!error) { return ""; }
  return `
    <div class="sp-settings__inline" role="alert">
      <span class="sp-badge sp-badge--err">${escapeHtml(t("personal-keys-badge-failed") || "Failed")}</span>
      <span class="sp-settings__error">${escapeHtml(error.message || "")}</span>
      ${fixButton(error.reason, provider)}
    </div>`;
}

function statusBadge(row) {
  return row.configured
    ? `<span class="sp-badge sp-badge--ok">${escapeHtml(t("personal-keys-badge-active") || "Key active")}</span>`
    : `<span class="sp-badge sp-badge--warn">${escapeHtml(t("personal-keys-badge-missing") || "No key")}</span>`;
}

function keyRow(row, ui) {
  const id = `pk-${escapeHtml(row.provider)}`;
  const name = escapeHtml(row.display_name || row.provider);
  const value = ui.editing ? (ui.draft || "") : (row.configured ? MASK : "");
  const busy = ui.busy ? "disabled aria-busy=\"true\"" : "";
  const remove = row.configured && !ui.editing
    ? `<button class="sp-btn-ghost" type="button" data-action="pk-remove" data-provider="${escapeHtml(row.provider)}" ${busy}>${escapeHtml(t("personal-keys-remove") || "Remove")}</button>`
    : "";
  return `
    <div class="sp-kv">
      <label for="${id}">${name} ${statusBadge(row)}</label>
      <div class="sp-settings__inline">
        <input id="${id}" class="sp-input sp-u-mono" type="password" autocomplete="off" spellcheck="false"
          value="${escapeHtml(value)}" data-input="pk-key" data-provider="${escapeHtml(row.provider)}"
          placeholder="${escapeHtml(t("personal-keys-placeholder") || "Paste your API key")}" ${busy}>
        <button class="sp-btn" type="button" data-action="pk-save" data-provider="${escapeHtml(row.provider)}"
          ${ui.editing && ui.draft ? busy : "disabled"}>${escapeHtml(t("personal-keys-save") || "Test & save")}</button>
        ${remove}
      </div>
      ${failureBlock(ui.error, row.provider)}
    </div>`;
}

export function renderPersonalKeys(component) {
  const heading = escapeHtml(t("personal-keys-heading") || "Personal subscriptions");
  const hint = escapeHtml(t("personal-keys-hint")
    || "Use your own subscription key for these providers. Each key is verified, stored encrypted, and used only for your requests.");
  if (component.loadError) {
    return `<section class="sp-settings__section"><h3>${heading}</h3>${failureBlock(component.loadError, "")}</section>`;
  }
  const rows = component.rows || [];
  if (!rows.length) { return ""; }
  return `
    <section class="sp-settings__section">
      <h3>${heading}</h3>
      <p class="sp-u-muted">${hint}</p>
      <div class="sp-kv__grid">${rows.map((r) => keyRow(r, component.ui(r.provider))).join("")}</div>
    </section>`;
}
