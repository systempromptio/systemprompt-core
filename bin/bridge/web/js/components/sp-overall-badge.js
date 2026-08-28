import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { isSetUp } from "/assets/js/utils/host-status.js";

function classify(snap, scope) {
  const cloudState = (snap.gateway_status && snap.gateway_status.state) || "unknown";
  if (cloudState === "probing" || cloudState === "unknown") {
    return { text: "checking…", cls: "sp-badge--muted" };
  }
  if (cloudState === "unreachable") {
    return { text: "cloud unreachable", cls: "sp-badge--err" };
  }
  return classifyHosts(snap, scope);
}

function classifyHosts(snap, scope) {
  let hosts = (snap.host_apps || []).filter((h) => h.enabled === true);
  // The Agents tab lists only agents that are set up, so a badge above that
  // list must judge the same set — otherwise it reads "profile not installed"
  // over a list in which everything is working, describing agents the reader
  // cannot see. The Status tab keeps the whole-instance view.
  if (scope === "set-up") { hosts = hosts.filter(isSetUp); }
  if (hosts.length === 0) {
    return scope === "set-up"
      ? { text: "no agents set up", cls: "sp-badge--muted" }
      : { text: "no agents enabled", cls: "sp-badge--muted" };
  }
  const proxyState = (snap.local_proxy && snap.local_proxy.state || "Unknown").toString();
  const anyAbsent = hosts.some((h) => (h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind || "absent") === "absent");
  const anyPartial = hosts.some((h) => h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind === "partial");
  const anyStale = hosts.some((h) => h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind === "stale");
  const allInstalled = hosts.every((h) => h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind === "installed");
  // Scoped to the Agents list, the badge summarises rows the reader can see, so
  // it uses their vocabulary; the Status tab keeps the diagnostic wording.
  const attention = scope === "set-up"
    ? { text: "needs attention", cls: "sp-badge--warn" }
    : null;
  if (anyStale) { return attention || { text: "secret out of date", cls: "sp-badge--warn" }; }
  if (anyAbsent) { return attention || { text: "profile not installed", cls: "sp-badge--warn" }; }
  if (anyPartial) { return attention || { text: "profile partial", cls: "sp-badge--warn" }; }
  if (allInstalled && proxyState === "Unconfigured") {
    return { text: "awaiting first launch", cls: "sp-badge--warn" };
  }
  if (allInstalled && proxyState === "Listening") {
    return { text: scope === "set-up" ? "all working" : "healthy", cls: "sp-badge--ok" };
  }
  if (allInstalled) {
    return { text: scope === "set-up" ? "not working" : "local proxy down", cls: "sp-badge--err" };
  }
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

  static get observedAttributes() { return ["scope"]; }

  attributeChangedCallback() { this.invalidate(); }

  render() {
    const v = classify(this.snapshot || {}, this.getAttribute("scope"));
    return `<span class="sp-badge ${v.cls}">${escapeHtml(v.text)}</span>`;
  }
}

reactive(SpOverallBadge.prototype, ["snapshot"]);
customElements.define("sp-overall-badge", SpOverallBadge);
