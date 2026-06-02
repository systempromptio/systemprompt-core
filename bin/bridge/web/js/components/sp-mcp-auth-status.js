import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { publishSectionState } from "/assets/js/utils/format.js";

// Maps a McpAuthState (serialized by Rust variant name) to display attributes.
const VIEWS = {
  Authenticated:       { s: "ok",      dot: "sp-dot--ok",      label: "authenticated" },
  NoServers:           { s: "warn",    dot: "sp-dot--warn",    label: "no servers registered" },
  LoopbackMismatch:    { s: "err",     dot: "sp-dot--err",     label: "bad loopback secret (403)" },
  GatewayUnauthorized: { s: "err",     dot: "sp-dot--err",     label: "gateway unauthorized (401)" },
  NotRegistered:       { s: "err",     dot: "sp-dot--err",     label: "not in proxy registry (404)" },
  UpstreamError:       { s: "err",     dot: "sp-dot--err",     label: "upstream error" },
  ProxyUnreachable:    { s: "err",     dot: "sp-dot--err",     label: "proxy unreachable" },
  ProtocolError:       { s: "err",     dot: "sp-dot--err",     label: "protocol error" },
  Unknown:             { s: "unknown", dot: "sp-dot--unknown", label: "not checked yet" },
};

function viewFor(state) {
  return VIEWS[state] || VIEWS.Unknown;
}

function rollUp(a, b) {
  const order = { err: 4, warn: 3, probing: 2, unknown: 1, ok: 0 };
  return (order[a] ?? 1) >= (order[b] ?? 1) ? a : b;
}

function sectionLabel(state) {
  if (state === "ok") { return "authenticated"; }
  if (state === "warn") { return "attention"; }
  if (state === "err") { return "failing"; }
  if (state === "probing") { return "checking…"; }
  return "unknown";
}

function detailRows(srv) {
  return [
    srv.url ? ["url", escapeHtml(srv.url)] : null,
    srv.session_id ? ["session", escapeHtml(srv.session_id)] : null,
    srv.http_status != null ? ["http", String(srv.http_status)] : null,
    srv.latency_ms != null ? ["latency", `${srv.latency_ms} ms`] : null,
    srv.error ? ["error", escapeHtml(srv.error)] : null,
  ].filter(Boolean);
}

function toolsBlock(srv) {
  if (srv.state !== "Authenticated") { return ""; }
  if (!srv.tools || srv.tools.length === 0) {
    return `<p class="sp-kpi-card__label">Authenticated — no tools exposed.</p>`;
  }
  const chips = srv.tools.map((t) => `<span class="sp-chip">${escapeHtml(t)}</span>`).join("");
  return `<p class="sp-kpi-card__label">Tools (${srv.tools.length})</p><div class="sp-chip-list sp-kpi-card__chips">${chips}</div>`;
}

function card(srv) {
  const view = viewFor(srv.state);
  const toolCount = srv.tools ? srv.tools.length : 0;
  const rows = detailRows(srv);
  const title = srv.id || "MCP servers";
  return `
    <article class="sp-kpi-card" data-state="${view.s}">
      <div class="sp-kpi-card__head">
        <span>${escapeHtml(title)}</span>
        <span class="sp-dot ${view.dot}" aria-hidden="true"></span>
      </div>
      <div class="sp-kpi-card__value">
        <span>${srv.state === "Authenticated" ? toolCount : "—"}</span>
        ${srv.state === "Authenticated" ? `<span class="sp-kpi-card__unit">${toolCount === 1 ? "tool" : "tools"}</span>` : ""}
      </div>
      <div class="sp-kpi-card__label">${escapeHtml(view.label)}</div>
      ${srv.session_id ? `<p class="sp-kpi-card__label">session <code>${escapeHtml(srv.session_id)}</code></p>` : ""}
      ${srv.error ? `<p class="sp-kpi-card__error">${escapeHtml(srv.error)}</p>` : ""}
      ${toolsBlock(srv)}
      ${rows.length ? `<details><summary>Details</summary><dl class="sp-kpi-card__details">${rows.map(([k, v]) => `<dt>${escapeHtml(k)}</dt><dd>${v}</dd>`).join("")}</dl></details>` : ""}
    </article>
  `;
}

export class SpMcpAuthStatus extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.registerAction("recheck", () => {
      bridge.invoke("mcp.auth.probe").catch((e) => console.warn("mcp.auth.probe failed", e));
    });
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this.bridgeSubscribe("mcp.changed", () => {
      bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    });
  }

  render() {
    const snap = this.snapshot || {};
    const servers = snap.mcp_auth || [];
    const probing = !!snap.mcp_auth_probe_in_flight;
    const recheck = `<button type="button" class="sp-btn sp-btn--ghost" data-action="recheck" ${probing ? "disabled" : ""}>${probing ? "Checking…" : "Recheck"}</button>`;

    if (servers.length === 0) {
      return `<div class="sp-kpi-grid">${card({ state: probing ? "Unknown" : "NoServers", tools: [] })}</div><div class="sp-kpi-card__foot"><span class="sp-kpi-card__foot-meta">Live round-trip through the loopback proxy.</span>${recheck}</div>`;
    }
    return `
      <div class="sp-kpi-grid">${servers.map(card).join("")}</div>
      <div class="sp-kpi-card__foot">
        <span class="sp-kpi-card__foot-meta">Live <code>initialize</code> + <code>tools/list</code> through the loopback proxy.</span>
        ${recheck}
      </div>
    `;
  }

  afterRender() {
    const snap = this.snapshot || {};
    const servers = snap.mcp_auth || [];
    const probing = !!snap.mcp_auth_probe_in_flight;
    // Derive the section state from the servers' worst state. (Don't seed with
    // "unknown" — rollUp ranks unknown above ok, so an authenticated server
    // would never lift the badge to green.)
    let overall;
    if (servers.length) {
      overall = servers.map((srv) => viewFor(srv.state).s).reduce(rollUp);
    } else {
      overall = probing ? "probing" : "warn";
    }
    publishSectionState(this, overall, sectionLabel(overall));
  }
}

reactive(SpMcpAuthStatus.prototype, ["snapshot"]);
customElements.define("sp-mcp-auth-status", SpMcpAuthStatus);
