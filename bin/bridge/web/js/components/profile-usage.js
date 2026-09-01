import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { fmtCompactNumber, fmtUsdMicros, fmtCostDelta } from "/assets/js/utils/profile-format.js";

function usageTile(label, w) {
  if (!w) { return ""; }
  const delta = fmtCostDelta(w.cost_microdollars, w.previous_cost_microdollars);
  return `
    <div class="sp-profile-tile">
      <div class="sp-profile-tile__label">${escapeHtml(label)}</div>
      <div class="sp-profile-tile__value">${escapeHtml(fmtUsdMicros(w.cost_microdollars))}</div>
      <div class="sp-profile-tile__sub">${escapeHtml(fmtCompactNumber(w.tokens))} tokens · ${escapeHtml(fmtCompactNumber(w.requests))} req</div>
      ${delta ? `<div class="sp-profile-tile__delta">${escapeHtml(delta)}</div>` : ""}
    </div>
  `;
}

export function renderProfileUsage(profile) {
  const u = profile.usage;
  if (!u) {
    return `
      <article class="sp-profile-card sp-profile-card--usage" data-state="empty">
        <header><h2 data-l10n-id="profile-section-usage">Token usage</h2></header>
        <p class="sp-u-muted" data-l10n-id="profile-usage-empty">No usage reported yet.</p>
      </article>
    `;
  }
  return `
    <article class="sp-profile-card sp-profile-card--usage">
      <header><h2 data-l10n-id="profile-section-usage">Token usage</h2></header>
      <div class="sp-profile-tiles">
        ${usageTile(t("profile-window-24h") || "Last 24 hours", u.d1)}
        ${usageTile(t("profile-window-7d") || "Last 7 days", u.d7)}
        ${usageTile(t("profile-window-30d") || "Last 30 days", u.d30)}
      </div>
    </article>
  `;
}

export function renderProfileModels(profile) {
  const top = (profile.usage && profile.usage.top_models) || [];
  if (top.length === 0) {
    return `
      <article class="sp-profile-card sp-profile-card--models" data-state="empty">
        <header><h2 data-l10n-id="profile-section-models">Favorite models</h2></header>
        <p class="sp-u-muted" data-l10n-id="profile-models-empty">No model usage in the last 30 days.</p>
      </article>
    `;
  }
  const rows = top.slice(0, 5).map((m, i) => `
    <li class="sp-profile-model" data-rank="${i + 1}">
      <span class="sp-profile-model__rank">#${i + 1}</span>
      <span class="sp-profile-model__name">${escapeHtml(m.model)}</span>
      <span class="sp-profile-model__share">${escapeHtml((Number(m.token_share || 0) * 100).toFixed(1))}%</span>
      <span class="sp-profile-model__tokens">${escapeHtml(fmtCompactNumber(m.tokens))} tokens</span>
      <span class="sp-profile-model__cost">${escapeHtml(fmtUsdMicros(m.cost_microdollars))}</span>
    </li>
  `).join("");
  return `
    <article class="sp-profile-card sp-profile-card--models">
      <header><h2 data-l10n-id="profile-section-models">Favorite models</h2></header>
      <ol class="sp-profile-models">${rows}</ol>
    </article>
  `;
}
