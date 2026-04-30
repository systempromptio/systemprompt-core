import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";

const KIND_LABEL = {
  plugins: "Plugin",
  skills: "Skill",
  hooks: "Hook",
  mcp: "MCP server",
  agents: "Agent",
};

export class SpMarketplaceDetail extends SpElement {
  constructor() {
    super();
    this.selected = null;
    this.kind = "plugins";
    this.copied = false;
    this.registerAction("copy-path", async () => {
      const value = this.selected && this.selected.path;
      if (!value) { return; }
      try {
        await navigator.clipboard.writeText(value);
        this.copied = true;
        setTimeout(() => { this.copied = false; }, 1200);
      } catch (e) {
        console.error("clipboard write failed", e);
      }
    });
  }

  render() {
    const selected = this.selected;
    if (!selected) {
      return `<article class="sp-mkt-detail">
        <div class="sp-mkt-empty">
          <span class="sp-mkt-empty__glyph" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
          </span>
          <span class="sp-mkt-empty__title" data-l10n-id="marketplace-empty-title">Select an item</span>
          <span class="sp-mkt-empty__sub">Pick from the list, or use <span class="sp-kbd">⌘F</span> to search.</span>
        </div>
      </article>`;
    }
    const sourceChip = selected.source ? `<span class="sp-mkt-chip">${escapeHtml(selected.source)}</span>` : "";
    const versionChip = selected.version ? `<span class="sp-mkt-chip sp-mkt-chip--mono">v${escapeHtml(selected.version)}</span>` : "";
    const summary = selected.summary ? `<p class="sp-mkt-detail__summary">${escapeHtml(selected.summary)}</p>` : "";
    const readme = selected.readme ? `<section class="sp-mkt-detail__section"><h3>${escapeHtml(t("marketplace-detail-readme") || "README")}</h3><div class="sp-mkt-detail__readme">${escapeHtml(selected.readme)}</div></section>` : "";
    const copyLabel = this.copied ? (t("marketplace-detail-copied") || "Copied") : (t("marketplace-detail-copy") || "Copy");
    const pathSection = selected.path ? `
      <section class="sp-mkt-detail__section">
        <h3>${escapeHtml(t("marketplace-detail-path") || "Path")}</h3>
        <div class="sp-mkt-detail__path-row">
          <span class="sp-mkt-detail__path">${escapeHtml(selected.path)}</span>
          <button type="button" class="sp-mkt-detail__copy" data-copied="${this.copied ? "true" : ""}" data-action="copy-path">${escapeHtml(copyLabel)}</button>
        </div>
      </section>` : "";
    return `<article class="sp-mkt-detail is-entering">
      <div class="sp-mkt-detail__head">
        <div class="sp-mkt-detail__title"><h2>${escapeHtml(selected.name || selected.id)}</h2></div>
      </div>
      <div class="sp-mkt-detail__meta">
        <span class="sp-mkt-chip" data-tone="accent">${escapeHtml(KIND_LABEL[this.kind] || this.kind)}</span>
        ${sourceChip}
        ${versionChip}
      </div>
      ${summary}
      ${readme}
      ${pathSection}
    </article>`;
  }
}

reactive(SpMarketplaceDetail.prototype, ["selected", "kind", "copied"]);
customElements.define("sp-marketplace-detail", SpMarketplaceDetail);
