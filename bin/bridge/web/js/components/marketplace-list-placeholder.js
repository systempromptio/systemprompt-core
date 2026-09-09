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

export function refreshErrorMarkup(error) {
  if (!error) { return ""; }
  return `<div class="sp-mkt-empty" role="status">
    <span class="sp-mkt-empty__title">${escapeHtml(t("marketplace-refresh-error") || "Could not refresh. Showing the previous list.")}</span>
    <span class="sp-mkt-empty__sub">${escapeHtml(error)}</span>
    <button class="sp-btn-ghost" type="button" data-action="retry">${escapeHtml(t("marketplace-retry") || "Try again")}</button>
  </div>`;
}

function idleState(reason) {
  const labels = {
    "signed-out": t("marketplace-signin-required") || "Sign in to view your marketplace.",
    "verifying": t("marketplace-verifying") || "Checking your connection…",
    "gateway-unreachable": t("marketplace-unreachable") || "Waiting for the gateway connection.",
  };
  return `<div class="sp-mkt-empty" role="status">${escapeHtml(labels[reason] || labels["verifying"])}</div>`;
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

export function placeholderMarkup({ state, error, search, kind, reason }) {
  if (state === "loading") { return skeleton(); }
  if (state === "idle") { return idleState(reason); }
  if (state === "error") { return errorState(error); }
  if (search) {
    return `<ul class="sp-mkt-items"><li class="sp-mkt-empty">
      <span class="sp-mkt-empty__title">${escapeHtml(t("marketplace-no-matches") || "No matches")}</span>
    </li></ul>`;
  }
  return emptyState(kind, reason);
}
