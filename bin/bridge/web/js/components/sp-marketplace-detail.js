import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { notifyOk, notifyErr } from "/assets/js/utils/notify.js";
import { bridge } from "/assets/js/bridge.js";
import { runAction } from "/assets/js/utils/action.js";
import { mktKindSingular } from "/assets/js/utils/marketplace-kinds.js";
import { renderMarketplaceMcp } from "/assets/js/components/marketplace-detail-mcp.js";
import { renderMarketplaceDetailEmpty, renderMarketplaceChildren, renderMarketplacePath } from "/assets/js/components/marketplace-detail-sections.js";

export class SpMarketplaceDetail extends SpElement {
  constructor() {
    super();
    this.selected = null;
    this.kind = "plugins";
    this.copied = false;
    this.knownIds = null;
    this.snapshot = null;
    this.registerAction("mcp-recheck", (trigger) => runAction(trigger, {
      run: () => bridge.mcpAuthProbe(this.selected && this.selected.id),
      success: () => t("mcp-rechecked") || "MCP server re-checked.",
      context: t("mcp-recheck") || "Re-check",
    }));
    this.registerAction("open-child", (trigger) => {
      this.dispatchEvent(new CustomEvent("mkt-navigate", {
        detail: { kind: trigger.dataset.kind, id: trigger.dataset.id },
        bubbles: true,
        composed: true,
      }));
    });
    this.registerAction("copy-path", () => this._copyPath());
  }

  async _copyPath() {
    const value = this.selected && this.selected.path;
    if (!value) { return; }
    try {
      await navigator.clipboard.writeText(value);
      this.copied = true;
      notifyOk(t("toast-copied") || "Copied to the clipboard.");
      setTimeout(() => { this.copied = false; }, 1200);
    } catch (e) {
      notifyErr(e, t("marketplace-detail-copy") || "Copy");
    }
  }

  render() {
    const selected = this.selected;
    if (!selected) { return renderMarketplaceDetailEmpty(); }
    const sourceChip = selected.source ? `<span class="sp-mkt-chip">${escapeHtml(selected.source)}</span>` : "";
    const versionChip = selected.version ? `<span class="sp-mkt-chip sp-mkt-chip--mono">v${escapeHtml(selected.version)}</span>` : "";
    const summary = selected.summary ? `<p class="sp-mkt-detail__summary">${escapeHtml(selected.summary)}</p>` : "";
    const readme = selected.readme ? `<section class="sp-mkt-detail__section"><h3>${escapeHtml(t("marketplace-detail-readme") || "README")}</h3><div class="sp-mkt-detail__readme">${escapeHtml(selected.readme)}</div></section>` : "";
    const mcpSection = this.kind === "mcp" ? renderMarketplaceMcp(this, selected) : "";
    return `<article class="sp-mkt-detail is-entering">
      <div class="sp-mkt-detail__head">
        <div class="sp-mkt-detail__title"><h2>${escapeHtml(selected.name || selected.id)}</h2></div>
      </div>
      <div class="sp-mkt-detail__meta">
        <span class="sp-mkt-chip">${escapeHtml(mktKindSingular(this.kind))}</span>
        ${sourceChip}
        ${versionChip}
      </div>
      ${summary}
      ${renderMarketplaceChildren(this, selected)}
      ${readme}
      ${mcpSection}
      ${renderMarketplacePath(this, selected)}
    </article>`;
  }
}

reactive(SpMarketplaceDetail.prototype, ["selected", "kind", "copied", "snapshot"]);
customElements.define("sp-marketplace-detail", SpMarketplaceDetail);
