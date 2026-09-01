import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { fmtCompactNumber, fmtIsoRelative } from "/assets/js/utils/profile-format.js";

function conversationGroup(label, arr) {
  const list = arr && arr.length
    ? `<ul class="sp-profile-group__list">
        ${arr.slice(0, 5).map((g) => `
          <li>
            <span class="sp-profile-group__name">${escapeHtml(g.name || "—")}</span>
            <span class="sp-profile-group__count">${escapeHtml(fmtCompactNumber(g.conversations))} conv · ${escapeHtml(fmtCompactNumber(g.ai_requests))} req</span>
          </li>
        `).join("")}
      </ul>`
    : `<p class="sp-u-muted" data-l10n-id="profile-none">none</p>`;
  return `
    <div class="sp-profile-group">
      <div class="sp-profile-group__label">${escapeHtml(label)}</div>
      ${list}
    </div>
  `;
}

function recentList(recent) {
  const items = (recent || []).slice(0, 5).map((r) => `
    <li class="sp-profile-recent">
      <span class="sp-profile-recent__id">${escapeHtml(r.context_id.slice(0, 12))}</span>
      <span class="sp-profile-recent__model">${escapeHtml(r.model || "—")}</span>
      <span class="sp-profile-recent__agent">${escapeHtml(r.agent_name || "—")}</span>
      <span class="sp-profile-recent__count">${escapeHtml(fmtCompactNumber(r.ai_requests))} req</span>
      <span class="sp-profile-recent__when">${escapeHtml(fmtIsoRelative(r.last_activity))}</span>
    </li>
  `).join("");
  if (!items) { return ""; }
  return `
    <div class="sp-profile-recent-wrap">
      <div class="sp-profile-group__label" data-l10n-id="profile-group-recent">Recent</div>
      <ul class="sp-profile-recents">${items}</ul>
    </div>
  `;
}

export function renderProfileConversations(profile) {
  const c = (profile.usage && profile.usage.conversations) || null;
  if (!c) {
    return `
      <article class="sp-profile-card sp-profile-card--conversations" data-state="empty">
        <header><h2 data-l10n-id="profile-section-conversations">Conversations</h2></header>
        <p class="sp-u-muted" data-l10n-id="profile-conversations-empty">No conversations recorded yet.</p>
      </article>
    `;
  }
  return `
    <article class="sp-profile-card sp-profile-card--conversations">
      <header>
        <h2 data-l10n-id="profile-section-conversations">Conversations</h2>
        <span class="sp-profile-card__count">${escapeHtml(fmtCompactNumber(c.total_conversations))} total · ${escapeHtml(fmtCompactNumber(c.total_ai_requests))} requests</span>
      </header>
      <div class="sp-profile-groups">
        ${conversationGroup(t("profile-group-by-model") || "By model", c.by_model)}
        ${conversationGroup(t("profile-group-by-agent") || "By agent", c.by_agent)}
      </div>
      ${recentList(c.recent)}
    </article>
  `;
}
