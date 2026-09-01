import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { badgeSuffix, fleetHeadline } from "/assets/js/utils/verdict.js";

// Cloud reachability is not an agent fact, so its tone still gates the badge
// here — but it only ever *precedes* the fleet verdict. Everything below that
// line is a lookup into the fold Rust already computed; this component used to
// re-derive it from `profile_state`, and disagreed with both the rows and the
// summary card.
function classify(snap, scope) {
  const gateway = snap.gateway_status || { tone: "unknown", code: "unknown" };
  if (!gateway.settled) {
    return { text: t(`gateway-state-${gateway.code}`) || "", cls: "sp-badge--muted" };
  }
  if (gateway.tone === "err") {
    return { text: t("agents-status-cloud-gateway-unreachable") || "", cls: "sp-badge--err" };
  }

  // The Agents tab lists only agents that are set up, so a badge above that
  // list must judge the same set — otherwise it describes agents the reader
  // cannot see. The Status tab keeps the whole-instance view.
  const fleets = snap.agent_fleet || {};
  const fleet = scope === "set-up" ? fleets.set_up : fleets.all;
  if (!fleet || fleet.total === 0) {
    return scope === "set-up"
      ? { text: "no agents set up", cls: "sp-badge--muted" }
      : { text: "no agents enabled", cls: "sp-badge--muted" };
  }
  return {
    text: fleetHeadline(fleet.headline) || "checking…",
    cls: `sp-badge--${badgeSuffix(fleet.state)}`,
  };
}

export class SpOverallBadge extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
  }

  onConnect() {
    this.useSnapshot((s) => { this.snapshot = s; });
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
