import { $, setDot } from "./dom.js?t=__TOKEN__";

function presenceState(host) {
  const kind = host.snapshot?.profile_state?.kind;
  if (kind === "installed" && host.snapshot?.host_running) {
    return "ok";
  } else if (kind === "installed") {
    return "warn";
  } else if (kind === "partial") {
    return "warn";
  } else if (kind === "absent") {
    return "err";
  } else {
    return "unknown";
  }
}

function presenceLabel(state) {
  if (state === "ok") {
    return "running";
  } else if (state === "warn") {
    return "needs attention";
  } else if (state === "err") {
    return "not installed";
  } else {
    return "unknown";
  }
}

export function renderAgentPresence(snap) {
  const cluster = $("agent-presence");
  if (cluster) {
    const list = snap.host_apps || [];
    cluster.replaceChildren();
    for (const host of list) {
      const dot = document.createElement("span");
      dot.className = "agent-dot";
      dot.dataset.action = "agent-jump";
      dot.dataset.agent = host.id;
      const state = presenceState(host);
      dot.dataset.state = state;
      dot.title = `${host.display_name} · ${presenceLabel(state)}`;
      cluster.append(dot);
    }
  }
}

export function renderAgentsSummary(snap) {
  const dot = $("agents-summary-dot");
  const text = $("agents-summary-text");
  if (dot && text) {
    const list = snap.host_apps || [];
    if (list.length === 0) {
      setDot(dot, "dot-unknown");
      text.textContent = "no agents registered";
    } else {
      const installed = list.filter((h) => h.snapshot?.profile_state?.kind === "installed").length;
      const running = list.filter((h) => h.snapshot?.host_running).length;
      const klass = installed === list.length ? "dot-ok" : installed > 0 ? "dot-warn" : "dot-err";
      setDot(dot, klass);
      text.textContent = `${installed} of ${list.length} agents configured · ${running} running`;
    }
  }
}

export function renderAgentsRailCount(snap) {
  const railCount = $("rail-count-agents");
  if (railCount) {
    const list = snap.host_apps || [];
    railCount.textContent = String(list.length);
  }
}
