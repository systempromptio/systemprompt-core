import { html, nothing } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";

function fmtRelative(unix) {
  if (!unix) { return "never"; }
  const delta = Math.max(0, Math.floor(Date.now() / 1000) - unix);
  if (delta < 5) { return "just now"; }
  if (delta < 60) { return `${delta}s ago`; }
  if (delta < 3600) { return `${Math.floor(delta / 60)}m ago`; }
  return `${Math.floor(delta / 3600)}h ago`;
}

function reachabilityView(status) {
  if (status.state === "reachable") {
    return { dot: "sp-dot--ok", label: `reachable · ${status.latency_ms}ms` };
  }
  if (status.state === "probing") {
    return { dot: "sp-dot--probing", label: "probing…" };
  }
  if (status.state === "unreachable") {
    return { dot: "sp-dot--err", label: `unreachable · ${status.reason || "unknown error"}` };
  }
  return { dot: "sp-dot--unknown", label: "unknown" };
}

function identityView(snap, reachable) {
  const id = snap.verified_identity;
  if (!reachable) {
    return { dot: "sp-dot--unknown", label: "(gateway unreachable)", muted: true };
  }
  if (id && (id.email || id.user_id)) {
    return { dot: "sp-dot--ok", label: id.email || id.user_id, muted: false };
  }
  if (snap.pat_present) {
    return { dot: "sp-dot--probing", label: "(verifying credentials…)", muted: true };
  }
  return { dot: "sp-dot--warn", label: "(not signed in)", muted: true };
}

export class SpCloudStatus extends BridgeElement {
  static properties = {
    snapshot: { state: true },
    recheckError: { state: true },
    logoutError: { state: true },
  };

  constructor() {
    super();
    this.snapshot = null;
    this.recheckError = "";
    this.logoutError = "";
  }

  createRenderRoot() {
    return this;
  }

  connectedCallback() {
    super.connectedCallback();
    bridge.stateSnapshot().then((snap) => { this.snapshot = snap; }).catch((e) => {
      console.error("sp-cloud-status snapshot failed", e);
    });
    this.bridgeSubscribe("state.changed", (snap) => { this.snapshot = snap; });
    this.bridgeSubscribe("gateway.changed", () => {
      bridge.stateSnapshot().then((snap) => { this.snapshot = snap; }).catch(() => {});
    });
  }

  async _onRecheck() {
    this.recheckError = "";
    try {
      await bridge.gatewayProbe();
    } catch (e) {
      this.recheckError = (e && e.message) || "probe failed";
    }
  }

  async _onLogout() {
    this.logoutError = "";
    try {
      await bridge.logout();
    } catch (e) {
      this.logoutError = (e && e.message) || "logout failed";
    }
  }

  render() {
    const snap = this.snapshot;
    if (!snap) {
      return html`
        <table class="sp-status__board">
          <tbody>
            <tr>
              <th>Reachability</th>
              <td>
                <div class="sp-status__row">
                  <span class="sp-dot sp-dot--probing" aria-hidden="true"></span>
                  <span>probing…</span>
                </div>
              </td>
              <td class="sp-status__actions"></td>
            </tr>
          </tbody>
        </table>
      `;
    }

    const status = snap.gateway_status || { state: "unknown" };
    const reach = reachabilityView(status);
    const ident = identityView(snap, status.state === "reachable");
    const id = snap.verified_identity;
    const tokenState = snap.cached_token
      ? `cached JWT • ${snap.cached_token.length} bytes • ttl ${snap.cached_token.ttl_seconds}s`
      : (snap.pat_present ? "PAT stored — JWT will refresh on next probe" : "no token");

    return html`
      <table class="sp-status__board">
        <tbody>
          <tr>
            <th>Reachability</th>
            <td>
              <div class="sp-status__row">
                <span class="sp-dot ${reach.dot}" aria-hidden="true"></span>
                <span>${reach.label}</span>
              </div>
              <div class="sp-status__detail sp-u-mono ${snap.gateway_url ? "" : "sp-u-muted"}">${snap.gateway_url || "—"}</div>
              <div class="sp-status__detail sp-u-muted">last probe <span>${fmtRelative(snap.last_probe_at_unix)}</span></div>
              ${this.recheckError ? html`<div class="sp-status__detail sp-u-muted">${this.recheckError}</div>` : nothing}
            </td>
            <td class="sp-status__actions">
              <button class="sp-btn-ghost" type="button" @click=${() => this._onRecheck()}>Re-check</button>
            </td>
          </tr>
          <tr>
            <th>Identity</th>
            <td>
              <div class="sp-status__row">
                <span class="sp-dot ${ident.dot}" aria-hidden="true"></span>
                <span class="sp-value ${ident.muted ? "sp-u-muted" : ""}">${ident.label}</span>
              </div>
              <div class="sp-status__detail sp-u-muted">
                user <span class="sp-u-mono">${(id && id.user_id) || "—"}</span> ·
                tenant <span class="sp-u-mono">${(id && id.tenant_id) || "—"}</span>
              </div>
              <div class="sp-status__detail sp-u-muted">
                token <span>${tokenState}</span>
              </div>
              ${this.logoutError ? html`<div class="sp-status__detail sp-u-muted">${this.logoutError}</div>` : nothing}
            </td>
            <td class="sp-status__actions">
              <button class="sp-btn-ghost" type="button" @click=${() => this._onLogout()}>Log out</button>
            </td>
          </tr>
        </tbody>
      </table>
    `;
  }
}

customElements.define("sp-cloud-status", SpCloudStatus);
