import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";

const KIND_EMPTY_L10N = {
  plugins: "marketplace-empty-plugins",
  skills: "marketplace-empty-skills",
  hooks: "marketplace-empty-hooks",
  mcp: "marketplace-empty-mcp",
  agents: "marketplace-empty-agents",
  artifacts: "marketplace-empty-artifacts",
};

const KIND_EMPTY_TITLE = {
  plugins: "No plugins yet",
  skills: "No skills yet",
  hooks: "No hooks yet",
  mcp: "No MCP servers yet",
  agents: "No agents yet",
  artifacts: "No artifacts yet",
};

function skeleton() {
  return `<ul class="sp-mkt-items" data-state="probing" aria-hidden="true">${
    [0, 1, 2, 3].map(() => `<li class="sp-mkt-item sp-mkt-item--skeleton" aria-hidden="true">
      <div class="sp-mkt-item__row"><span class="sp-mkt-item__name">&nbsp;</span></div>
      <div class="sp-mkt-item__meta">&nbsp;</div>
    </li>`).join("")
  }</ul>`;
}

function errorState(error) {
  return `<ul class="sp-mkt-items"><li class="sp-mkt-empty">
    <span class="sp-mkt-empty__title">${escapeHtml(t("marketplace-error-title") || "Could not load this list")}</span>
    <span class="sp-mkt-empty__sub">${escapeHtml(error || "")}</span>
    <button class="sp-btn-ghost" type="button" data-action="retry">${escapeHtml(t("marketplace-retry") || "Try again")}</button>
  </li></ul>`;
}

function emptyState(kind, reason) {
  const neverSynced = reason === "never-synced";
  return `<ul class="sp-mkt-items"><li class="sp-mkt-empty--with-sync">
    <span class="sp-mkt-empty__title">${escapeHtml(t(KIND_EMPTY_L10N[kind]) || KIND_EMPTY_TITLE[kind]
      || t("marketplace-empty-generic") || "Nothing here yet")}</span>
    <span class="sp-mkt-empty__sub">${escapeHtml(
      neverSynced
        ? (t("marketplace-empty-never-synced") || "Sync to pull what your account already has.")
        : (t("marketplace-empty-synced") || "Your last sync did not include anything of this kind."))}</span>
    ${neverSynced
      ? `<button class="sp-btn-primary" type="button" data-action="sync">${escapeHtml(t("sync-button") || "Sync now")}</button>`
      : ""}
  </li></ul>`;
}

// Why: loading, empty and broken used to render the same line. They are three
// different situations and the only ones the user can act on are the last two.
export function placeholderMarkup({ state, error, search, kind, reason }) {
  if (state === "loading" || state === "idle") { return skeleton(); }
  if (state === "error") { return errorState(error); }
  if (search) {
    return `<ul class="sp-mkt-items"><li class="sp-mkt-empty">
      <span class="sp-mkt-empty__title">${escapeHtml(t("marketplace-no-matches") || "No matches")}</span>
    </li></ul>`;
  }
  return emptyState(kind, reason);
}
