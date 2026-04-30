import { $, setDot } from "./dom.js?t=__TOKEN__";

function fmtCount(n) {
  const v = Number(n) || 0;
  if (v >= 1_000_000) {
    return `${(v / 1_000_000).toFixed(1)}M`;
  } else if (v >= 1_000) {
    return `${(v / 1_000).toFixed(1)}k`;
  } else {
    return String(v);
  }
}

function syncPillStateForFooter(snap) {
  if (snap.sync_in_flight) {
    return "running";
  } else if (snap.gateway_status && snap.gateway_status.state === "unreachable") {
    return "err";
  } else if (snap.signed_in) {
    return "ok";
  } else {
    return "idle";
  }
}

function dotClass(state) {
  if (state === "ok") {
    return "sp-dot--ok";
  } else if (state === "running") {
    return "sp-dot--probing";
  } else if (state === "err") {
    return "sp-dot--err";
  } else {
    return "sp-dot--warn";
  }
}

export function renderFooter(snap) {
  const config = $("config-path");
  if (config) {
    config.textContent = snap.config_file || "";
  }
  const hostDot = $("footer-host-dot");
  if (hostDot) {
    setDot(hostDot, dotClass(syncPillStateForFooter(snap)));
  }
  const stats = snap.proxy_stats || {};
  const msgs = $("lane-msgs");
  if (msgs) {
    msgs.textContent = fmtCount(stats.messages_total);
  }
  const tin = $("lane-tin");
  if (tin) {
    tin.textContent = fmtCount(stats.tokens_in_total);
  }
  const tout = $("lane-tout");
  if (tout) {
    tout.textContent = fmtCount(stats.tokens_out_total);
  }

  const pluginsDir = $("plugins-dir");
  if (pluginsDir) {
    pluginsDir.textContent = snap.plugins_dir || "—";
  }
  const lastSync = $("last-sync");
  if (lastSync) {
    lastSync.textContent = snap.last_sync_summary || "never synced";
  }
  const mktMeta = $("mkt-meta");
  if (mktMeta) {
    const state = snap.last_sync_summary
      ? (snap.gateway_status && snap.gateway_status.state === "unreachable" ? "err" : "ok")
      : "never";
    mktMeta.dataset.state = state;
    mktMeta.title = snap.last_sync_summary
      ? `Last sync: ${snap.last_sync_summary}\n${snap.plugins_dir || ""}`
      : (snap.plugins_dir || "Run a sync to populate");
  }
  const settingsGateway = $("settings-gateway");
  if (settingsGateway) {
    settingsGateway.textContent = snap.gateway_url || "—";
    settingsGateway.classList.toggle("sp-u-muted", !snap.gateway_url);
  }
  const settingsPlugins = $("settings-plugins-dir");
  if (settingsPlugins) {
    settingsPlugins.textContent = snap.plugins_dir || "—";
    settingsPlugins.classList.toggle("sp-u-muted", !snap.plugins_dir);
  }
  const settingsConfig = $("settings-config");
  if (settingsConfig) {
    settingsConfig.textContent = snap.config_file || "—";
    settingsConfig.classList.toggle("sp-u-muted", !snap.config_file);
  }
  const btnSync = $("btn-sync");
  if (btnSync) {
    btnSync.disabled = !!snap.sync_in_flight || !snap.signed_in;
  }
  const btnConnect = $("setup-connect");
  if (btnConnect) {
    btnConnect.disabled = !!snap.sync_in_flight;
  }
}
