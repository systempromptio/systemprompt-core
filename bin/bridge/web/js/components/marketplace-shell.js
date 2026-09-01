import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { shortcut } from "/assets/js/utils/rail-tabs.js";
import { MKT_KINDS } from "/assets/js/services/marketplace-service.js";
import { MKT_KIND, MKT_KIND_L10N } from "/assets/js/utils/marketplace-kinds.js";

function diffSummary(diff) {
  if (!diff) { return ""; }
  const parts = [];
  if (diff.installed && diff.installed.length) { parts.push(`${diff.installed.length} added`); }
  if (diff.updated && diff.updated.length) { parts.push(`${diff.updated.length} updated`); }
  if (diff.removed && diff.removed.length) { parts.push(`${diff.removed.length} removed`); }
  if (parts.length === 0) { return "no changes since last sync"; }
  return parts.join(", ");
}

// The footer used to report the snapshot's counts while the list beside it
// reported the listing's, and the two disagreed on screen. Once the listing is
// loaded it is the only source; before that the snapshot's line is labelled as
// what it is.
function listingSummary(listing) {
  const parts = [];
  for (const k of MKT_KINDS) {
    const n = ((listing && listing[k]) || []).length;
    if (n) { parts.push(`${n} ${k}`); }
  }
  return parts.length ? parts.join(", ") : t("marketplace-empty-generic") || "Nothing here yet";
}

function badgeView(snap) {
  if (!snap.signed_in) { return { text: t("marketplace-badge-signin") || "sign in", cls: "sp-badge--warn" }; }
  if (snap.sync_in_flight) { return { text: t("marketplace-badge-syncing") || "syncing", cls: "sp-badge--warn" }; }
  if (snap.last_sync_summary) { return { text: t("marketplace-badge-synced") || "synced", cls: "sp-badge--ok" }; }
  return { text: t("marketplace-badge-never") || "never synced", cls: "sp-badge--muted" };
}

export function renderMarketplaceHeader(snap) {
  const badge = badgeView(snap);
  return `
    <header class="sp-tab__header">
      <h1 data-l10n-id="marketplace-heading">Marketplace</h1>
      <span class="sp-badge ${badge.cls}">${escapeHtml(badge.text)}</span>
    </header>`;
}

// A real <button>: role="tab" on an <li> alongside an aria-hidden <li> label
// is not a valid tablist, and every tab carried tabindex="0" with no key
// handler, so the rail was five tab stops that did nothing.
export function renderMarketplaceCats(component, counts) {
  const cats = MKT_KINDS.map((k) => {
    const selected = component.kind === k;
    const kind = MKT_KIND[k];
    return `
      <li role="presentation">
        <button class="sp-mkt-cat" type="button" data-kind="${k}" role="tab" id="sp-mkt-cat-${k}" aria-controls="sp-mkt-items" aria-selected="${selected ? "true" : "false"}" tabindex="${selected ? "0" : "-1"}" data-action="select-kind">
          <span class="sp-mkt-cat__glyph" aria-hidden="true">${kind.glyph}</span>
          <span class="sp-mkt-cat__name" data-l10n-id="${MKT_KIND_L10N[k]}">${escapeHtml(kind.label)}</span>
          <span class="sp-mkt-cat__count ${counts[k] === 0 ? "is-zero" : ""}">${counts[k] === null ? "—" : counts[k]}</span>
        </button>
      </li>`;
  }).join("");
  return `
    <ul class="sp-mkt-cats" role="tablist" aria-orientation="vertical" data-l10n-aria="marketplace-categories-aria" aria-label="Marketplace categories">
      <li class="sp-mkt-cats__label" aria-hidden="true" data-l10n-id="marketplace-categories">Categories</li>
      ${cats}
    </ul>`;
}

export function renderMarketplaceSearch() {
  return `
    <label class="sp-mkt-search__wrap">
      <svg class="sp-mkt-search__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
      <input id="mkt-search" class="sp-mkt-search" type="search" placeholder="Search…" data-l10n-placeholder="marketplace-search-placeholder" autocomplete="off" spellcheck="false" data-input="search" />
      <span class="sp-mkt-search__kbd" aria-hidden="true">${escapeHtml(shortcut("F"))}</span>
    </label>`;
}

export function renderMarketplaceFooter(component, loaded) {
  const snap = component.snapshot || {};
  const syncDisabled = snap.sync_in_flight || !snap.signed_in;
  const mktState = snap.last_sync_summary ? "ok" : "never";
  const diff = (component.listing && component.listing.last_sync_diff) || null;
  const diffLine = diff ? diffSummary(diff) : "";
  return `
    <footer class="sp-mkt-actions">
      <button class="sp-btn-primary" type="button" data-l10n-id="sync-button" ${syncDisabled ? "disabled" : ""} data-action="sync">Sync now</button>
      <button class="sp-btn-ghost" type="button" data-l10n-id="marketplace-action-validate" data-action="validate">Validate</button>
      <button class="sp-btn-ghost" type="button" data-l10n-id="marketplace-action-open-folder" data-action="open-folder">Open folder</button>
      <span class="sp-mkt-actions__meta" data-state="${mktState}" title="${escapeHtml(snap.last_sync_summary || "—")}">
        <span class="sp-dot" aria-hidden="true"></span>
        <span>${escapeHtml(loaded ? listingSummary(component.listing) : (snap.last_sync_summary || t("last-sync-never")))}</span>
        ${diffLine ? `<span class="sp-mkt-actions__diff">· ${escapeHtml(diffLine)}</span>` : ""}
      </span>
    </footer>`;
}
