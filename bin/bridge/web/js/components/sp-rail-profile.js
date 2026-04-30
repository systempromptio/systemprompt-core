import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";

const VERSION = (() => {
  const tag = document.querySelector('meta[name="sp-version"]');
  return (tag && tag.content) || "";
})();

function initials(idSrc) {
  const letters = (idSrc || "").replace(/[^a-zA-Z]/g, "").slice(0, 2).toUpperCase();
  return letters || "SP";
}

export class SpRailProfile extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this._baseVersion = "";
  }

  onConnect() {
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
    return `
      <span class="sp-avatar__mark" aria-hidden="true"><span>${escapeHtml(initials(id && (id.email || id.user_id)))}</span></span>
      <span class="sp-rail-profile__meta">
        <span class="sp-rail-profile__id">${escapeHtml(idLabel)}</span>
        <span class="sp-rail-profile__sub">${escapeHtml(sub)}</span>
      </span>
    `;
  }
}

reactive(SpRailProfile.prototype, ["snapshot"]);
customElements.define("sp-rail-profile", SpRailProfile);
