import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { shortcut } from "/assets/js/utils/rail-tabs.js";
import { MKT_CHILD_KIND_ORDER, mktKindSingular } from "/assets/js/utils/marketplace-kinds.js";

export function renderMarketplaceDetailEmpty() {
  return `<article class="sp-mkt-detail">
    <div class="sp-mkt-empty">
      <span class="sp-mkt-empty__glyph" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
      </span>
      <span class="sp-mkt-empty__title" data-l10n-id="marketplace-empty-title">Select an item</span>
      <span class="sp-mkt-empty__sub">Pick from the list, or use <span class="sp-kbd">${escapeHtml(shortcut("F"))}</span> to search.</span>
    </div>
  </article>`;
}

function childButton(component, kind, child) {
  const ids = component.knownIds && component.knownIds[kind];
  const known = !!(ids && ids.has(child.id));
  const sharedChip = child.shared ? `<span class="sp-mkt-chip">shared</span>` : "";
  const kindChip = `<span class="sp-mkt-chip">${escapeHtml(mktKindSingular(kind))}</span>`;
  return `<button type="button" class="sp-mkt-child" data-kind="${escapeHtml(kind)}" data-id="${escapeHtml(child.id)}" ${known ? `data-action="open-child"` : "disabled"}>
    <span class="sp-mkt-child__name">${escapeHtml(child.name || child.id)}</span>
    ${sharedChip}${kindChip}
  </button>`;
}

export function renderMarketplaceChildren(component, selected) {
  const children = Array.isArray(selected.children) ? selected.children : [];
  if (!children.length) { return ""; }
  const buttons = MKT_CHILD_KIND_ORDER.flatMap((kind) =>
    children.filter((c) => c.kind === kind).map((c) => childButton(component, kind, c)),
  ).join("");
  return `
    <section class="sp-mkt-detail__section">
      <h3>${escapeHtml(t("marketplace-detail-contents") || "Contents")} (${children.length})</h3>
      <div class="sp-mkt-detail__children">${buttons}</div>
    </section>`;
}

export function renderMarketplacePath(component, selected) {
  if (!selected.path) { return ""; }
  const copyLabel = component.copied
    ? (t("marketplace-detail-copied") || "Copied")
    : (t("marketplace-detail-copy") || "Copy");
  return `
    <section class="sp-mkt-detail__section">
      <h3>${escapeHtml(t("marketplace-detail-path") || "Path")}</h3>
      <div class="sp-mkt-detail__path-row">
        <span class="sp-mkt-detail__path">${escapeHtml(selected.path)}</span>
        <button type="button" class="sp-mkt-detail__copy" data-copied="${component.copied ? "true" : ""}" data-action="copy-path">${escapeHtml(copyLabel)}</button>
      </div>
    </section>`;
}
