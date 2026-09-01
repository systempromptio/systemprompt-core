import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { TAB_DEFS, shortcut } from "/assets/js/utils/rail-tabs.js";
import { LOG_LEVELS, logLevelLabel } from "/assets/js/utils/log-format.js";

export function renderActivityControls(component) {
  const levelBtns = LOG_LEVELS.map((lv) => {
    const pressed = component._level === lv ? "true" : "false";
    return `<button type="button" class="sp-btn-ghost sp-log__filter" data-action="set-level" data-level="${lv}" aria-pressed="${pressed}">${escapeHtml(logLevelLabel(lv))}</button>`;
  }).join("");
  const placeholder = escapeHtml(t("activity-search-placeholder") || "Filter activity…");
  return `
    <div class="sp-log__controls">
      <input type="search" class="sp-log__search" data-input="search" value="${escapeHtml(component._query)}"
        placeholder="${placeholder}" aria-label="${placeholder}">
      <div class="sp-log__filters" role="group">${levelBtns}</div>
      <button type="button" class="sp-btn-ghost" data-action="copy">${escapeHtml(t("activity-copy") || "Copy")}</button>
    </div>
  `;
}

export function renderActivityHelp() {
  const tabShortcuts = TAB_DEFS.map((d) => `
    <dt><kbd class="sp-kbd">${escapeHtml(shortcut(d.key))}</kbd></dt>
    <dd>${escapeHtml(t(d.l10n) || d.label)}</dd>`).join("");
  return `
    <section class="sp-activity__help" data-l10n-aria="activity-help-aria" aria-label="Help and support">
      <header class="sp-activity__help-title" data-l10n-id="activity-help-title">Help &amp; Support</header>
      <div class="sp-activity__help-actions">
        <button class="sp-btn-ghost" type="button" data-l10n-id="activity-open-log-folder" data-action="open-log-folder">Open log folder</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="activity-export-bundle" data-action="export-bundle">Export diagnostic bundle</button>
      </div>
      <details class="sp-activity__shortcuts">
        <summary data-l10n-id="activity-shortcuts-title">Keyboard shortcuts</summary>
        <dl class="sp-shortcuts">
          ${tabShortcuts}
          <dt><kbd class="sp-kbd">${escapeHtml(shortcut("F"))}</kbd></dt>
          <dd>${escapeHtml(t("activity-shortcut-search") || "Search the marketplace")}</dd>
          <dt><kbd class="sp-kbd">Esc</kbd></dt>
          <dd>${escapeHtml(t("activity-shortcut-escape") || "Close the open panel")}</dd>
        </dl>
      </details>
    </section>
  `;
}

export function renderActivityHeader() {
  return `
    <header class="sp-activity__header">
      <span class="sp-activity__title" data-l10n-id="activity-title">Activity</span>
      <div class="sp-activity-lane" data-l10n-aria="activity-totals-aria" aria-label="Activity totals">
        <span class="sp-activity-lane__stat"><b data-stat="msgs" data-preserve>0</b><span class="sp-activity-lane__label" data-l10n-id="activity-msgs">msgs</span></span>
        <span class="sp-activity-lane__stat"><b data-stat="tin" data-preserve>0</b><span class="sp-activity-lane__label" data-l10n-id="activity-tin">in</span></span>
        <span class="sp-activity-lane__stat"><b data-stat="tout" data-preserve>0</b><span class="sp-activity-lane__label" data-l10n-id="activity-tout">out</span></span>
      </div>
    </header>
  `;
}
