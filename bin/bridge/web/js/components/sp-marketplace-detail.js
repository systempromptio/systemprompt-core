import { html, nothing } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { t } from "/assets/js/i18n.js";

const KIND_LABEL = {
  plugins: "Plugin",
  skills: "Skill",
  hooks: "Hook",
  mcp: "MCP server",
  agents: "Agent",
};

export class SpMarketplaceDetail extends BridgeElement {
  static properties = {
    selected: { attribute: false },
    kind: { attribute: false },
    copied: { state: true },
  };

  constructor() {
    super();
    this.selected = null;
    this.kind = "plugins";
    this.copied = false;
  }

  createRenderRoot() { return this; }

  async _copy(value) {
    try {
      await navigator.clipboard.writeText(value);
      this.copied = true;
      setTimeout(() => { this.copied = false; }, 1200);
    } catch (e) {
      console.error("clipboard write failed", e);
    }
  }

  render() {
    const selected = this.selected;
    if (!selected) {
      return html`<article class="sp-mkt-detail">
        <div class="sp-mkt-empty">
          <span class="sp-mkt-empty__glyph" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
          </span>
          <span class="sp-mkt-empty__title" data-l10n-id="marketplace-empty-title">Select an item</span>
          <span class="sp-mkt-empty__sub">Pick from the list, or use <span class="sp-kbd">⌘F</span> to search.</span>
        </div>
      </article>`;
    }
    return html`<article class="sp-mkt-detail is-entering">
      <div class="sp-mkt-detail__head">
        <div class="sp-mkt-detail__title"><h2>${selected.name || selected.id}</h2></div>
      </div>
      <div class="sp-mkt-detail__meta">
        <span class="sp-mkt-chip" data-tone="accent">${KIND_LABEL[this.kind] || this.kind}</span>
        ${selected.source ? html`<span class="sp-mkt-chip">${selected.source}</span>` : nothing}
        ${selected.version ? html`<span class="sp-mkt-chip sp-mkt-chip--mono">v${selected.version}</span>` : nothing}
      </div>
      ${selected.summary ? html`<p class="sp-mkt-detail__summary">${selected.summary}</p>` : nothing}
      ${selected.readme ? html`<section class="sp-mkt-detail__section">
        <h3>${t("marketplace-detail-readme") || "README"}</h3>
        <div class="sp-mkt-detail__readme">${selected.readme}</div>
      </section>` : nothing}
      ${selected.path ? html`<section class="sp-mkt-detail__section">
        <h3>${t("marketplace-detail-path") || "Path"}</h3>
        <div class="sp-mkt-detail__path-row">
          <span class="sp-mkt-detail__path">${selected.path}</span>
          <button type="button" class="sp-mkt-detail__copy" data-copied=${this.copied ? "true" : ""} @click=${(e) => { e.stopPropagation(); this._copy(selected.path); }}>${this.copied ? (t("marketplace-detail-copied") || "Copied") : (t("marketplace-detail-copy") || "Copy")}</button>
        </div>
      </section>` : nothing}
    </article>`;
  }
}

customElements.define("sp-marketplace-detail", SpMarketplaceDetail);
