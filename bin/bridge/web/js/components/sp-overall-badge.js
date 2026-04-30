import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";

function classify(snap) {
  const cloudState = (snap.gateway_status && snap.gateway_status.state) || "unknown";
  if (cloudState === "probing" || cloudState === "unknown") {
    return { text: "checking…", cls: "sp-badge--muted" };
  }
  if (cloudState === "unreachable") {
    return { text: "cloud unreachable", cls: "sp-badge--err" };
  }
  return classifyHosts(snap);
}

function classifyHosts(snap) {
  const hosts = snap.host_apps || [];
  if (hosts.length === 0) { return { text: "no host apps", cls: "sp-badge--muted" }; }
  const proxyState = (snap.local_proxy && snap.local_proxy.state || "Unknown").toString();
  const anyAbsent = hosts.some((h) => (h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind || "absent") === "absent");
  const anyPartial = hosts.some((h) => h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind === "partial");
  const allInstalled = hosts.every((h) => h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind === "installed");
  if (anyAbsent) { return { text: "profile not installed", cls: "sp-badge--warn" }; }
  if (anyPartial) { return { text: "profile partial", cls: "sp-badge--warn" }; }
  if (allInstalled && proxyState === "Unconfigured") {
    return { text: "awaiting first launch", cls: "sp-badge--warn" };
  }
  if (allInstalled && proxyState === "Listening") { return { text: "healthy", cls: "sp-badge--ok" }; }
  if (allInstalled) { return { text: "local proxy down", cls: "sp-badge--err" }; }
  return { text: "checking…", cls: "sp-badge--muted" };
}

export class SpOverallBadge extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
  }

  render() {
    const v = classify(this.snapshot || {});
    return `<span class="sp-badge ${v.cls}">${escapeHtml(v.text)}</span>`;
  }
}

reactive(SpOverallBadge.prototype, ["snapshot"]);
customElements.define("sp-overall-badge", SpOverallBadge);
