import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { publishSectionState } from "/assets/js/utils/format.js";
import { toneSection } from "/assets/js/utils/verdict.js";
import { logout } from "/assets/js/services/session-service.js";
import {
  renderCloudReachCard, renderCloudIdentityCard, renderCloudSkeleton,
} from "/assets/js/components/cloud-status-sections.js";

export class SpCloudStatus extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.recheckError = "";
    this.logoutError = "";
    this.registerAction("recheck", () => this._onRecheck());
    this.registerAction("logout", () => this._onLogout());
  }

  onConnect() {
    this.useSnapshot((snap) => { this.snapshot = snap; });
  }

  async _onRecheck() {
    this.recheckError = "";
    try { await bridge.gatewayProbe(); }
    catch (e) { this.recheckError = (e && e.message) || "probe failed"; }
  }

  async _onLogout() {
    this.logoutError = await logout();
  }

  render() {
    const snap = this.snapshot;
    if (!snap) {
      return renderCloudSkeleton();
    }
    return `
      <div class="sp-kpi-grid">
        ${renderCloudReachCard(snap, this.recheckError)}
        ${renderCloudIdentityCard(snap, this.logoutError)}
      </div>
    `;
  }

  afterRender() {
    const snap = this.snapshot;
    if (!snap) {
      publishSectionState(this, "probing", "checking…");
      return;
    }
    const overall = snap.cloud_tone || "unknown";
    publishSectionState(this, overall, toneSection(overall));
  }
}

reactive(SpCloudStatus.prototype, ["snapshot", "recheckError", "logoutError"]);
customElements.define("sp-cloud-status", SpCloudStatus);
