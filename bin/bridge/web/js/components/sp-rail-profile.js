import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";

const VERSION = (() => {
  const tag = document.querySelector('meta[name="sp-version"]');
  return (tag && tag.content) || "";
})();

function initials(idSrc) {
  const letters = (idSrc || "").replace(/[^a-zA-Z]/g, "").slice(0, 2).toUpperCase();
  return letters || "SP";
}

export class SpRailProfile extends BridgeElement {
  static properties = { snapshot: { state: true } };

  constructor() {
    super();
    this.snapshot = null;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    this.classList.add("sp-rail-profile");
    this.setAttribute("aria-label", "Profile and workspace");
    if (!this._baseVersion) {
      this._baseVersion = this.dataset.version || VERSION || "";
    }
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
  }

  render() {
    const id = (this.snapshot && this.snapshot.verified_identity) || null;
    const tenant = id && id.tenant_id;
    const idLabel = (id && (id.email || id.user_id)) || "cowork workspace";
    const subBase = this._baseVersion;
    const sub = tenant ? `${tenant} · ${subBase}` : subBase;
    return html`
      <span class="sp-avatar__mark" aria-hidden="true"><span>${initials(id && (id.email || id.user_id))}</span></span>
      <span class="sp-rail-profile__meta">
        <span class="sp-rail-profile__id">${idLabel}</span>
        <span class="sp-rail-profile__sub">${sub}</span>
      </span>
    `;
  }
}

customElements.define("sp-rail-profile", SpRailProfile);
