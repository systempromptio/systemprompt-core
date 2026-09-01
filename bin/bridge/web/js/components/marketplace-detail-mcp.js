import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { fmtRelative } from "/assets/js/utils/format.js";
import { toneDot } from "/assets/js/utils/verdict.js";

function mcpFacts(snap, srv, extra) {
  const id = snap.verified_identity || {};
  const who = id.email || id.user_id || "";
  const identity = snap.identity || { tone: "unknown", code: "signed-out" };
  const identityText = identity.tone === "ok"
    ? (t("mcp-signed-in-as", { email: who }) || who)
    : (t(`identity-${identity.code}`) || "");
  return [
    [t("status-cloud-identity-label") || "Identity",
      `<span class="sp-dot ${toneDot(identity.tone)}" aria-hidden="true"></span> ${escapeHtml(identityText)}`],
    srv && srv.http_status != null ? ["http", escapeHtml(String(srv.http_status))] : null,
    srv && srv.latency_ms != null ? ["latency", `${escapeHtml(String(srv.latency_ms))} ms`] : null,
    srv && srv.probed_at_unix ? [t("mcp-checked") || "checked", escapeHtml(fmtRelative(srv.probed_at_unix))] : null,
    srv && srv.session_id ? ["session", `<code>${escapeHtml(srv.session_id)}</code>`] : null,
    extra.proxy_url ? [t("mcp-proxy-url") || "Proxy URL", `<code>${escapeHtml(extra.proxy_url)}</code>`] : null,
    extra.upstream_url ? [t("mcp-upstream-url") || "Upstream URL", `<code>${escapeHtml(extra.upstream_url)}</code>`] : null,
  ].filter(Boolean);
}

function mcpToolsSection(srv) {
  const tools = (srv && srv.tools) || [];
  const shows = !!(srv && srv.shows_tools);
  let body = `<p class="sp-u-muted">${escapeHtml(t("mcp-tools-unavailable") || "")}</p>`;
  if (shows && tools.length) {
    body = `<ul class="sp-mkt-tools">${tools.map((tool) => `
      <li class="sp-mkt-tool">
        <code class="sp-mkt-tool__name">${escapeHtml(tool.name)}</code>
        ${tool.description ? `<span class="sp-mkt-tool__desc">${escapeHtml(tool.description)}</span>` : ""}
      </li>`).join("")}</ul>`;
  } else if (shows) {
    body = `<p class="sp-u-muted">${escapeHtml(t("mcp-no-tools") || "")}</p>`;
  }
  return `
    <section class="sp-mkt-detail__section">
      <h3>${escapeHtml(t("marketplace-detail-tools") || "Tools")}${shows ? ` (${tools.length})` : ""}</h3>
      ${body}
    </section>`;
}

// Everything about one MCP server on one screen: the bridge's live auth
// verdict for it, who it is authenticated as, the tools `tools/list` returned,
// and both URLs. The listing carries identity only; the live row is the
// snapshot's, so this never shows a probe older than the Status pane's.
export function renderMarketplaceMcp(component, selected) {
  const snap = component.snapshot || {};
  const srv = (snap.mcp_auth || []).find((s) => s.id === selected.id) || null;
  const probing = !!snap.mcp_auth_probe_in_flight;
  const verdict = (srv && srv.verdict) || { tone: "unknown", code: "unknown" };
  const recheckLabel = probing ? (t("mcp-checking") || "Checking…") : (t("mcp-recheck") || "Re-check");
  const facts = mcpFacts(snap, srv, selected.extra || {});
  return `
    <section class="sp-mkt-detail__section sp-mkt-mcp" data-state="${escapeHtml(verdict.tone)}">
      <h3>${escapeHtml(t("marketplace-detail-auth") || "Authentication")}</h3>
      <div class="sp-status__row">
        <span class="sp-dot ${toneDot(verdict.tone)}" aria-hidden="true"></span>
        <span>${escapeHtml(t(`mcp-auth-${verdict.code}`) || "")}</span>
        <button type="button" class="sp-btn-ghost sp-mkt-mcp__recheck" data-action="mcp-recheck" ${probing ? "disabled" : ""}>${escapeHtml(recheckLabel)}</button>
      </div>
      ${srv && srv.error ? `<p class="sp-kpi-card__error">${escapeHtml(srv.error)}</p>` : ""}
      <dl class="sp-kpi-card__details">${facts.map(([k, v]) => `<dt>${escapeHtml(k)}</dt><dd>${v}</dd>`).join("")}</dl>
    </section>
    ${mcpToolsSection(srv)}`;
}
