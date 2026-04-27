(function () {
  const $ = (id) => document.getElementById(id);
  const TOKEN = "__TOKEN__";

  const STATE_POLL_MS = 1500;
  const LOG_POLL_MS   = 1000;

  let logCursor = 0;

  function api(path, init) {
    const sep = path.includes("?") ? "&" : "?";
    return fetch(`${path}${sep}t=${encodeURIComponent(TOKEN)}`, init);
  }

  async function post(path, body) {
    try {
      const resp = await api(path, {
        method: "POST",
        headers: body ? { "Content-Type": "application/json" } : {},
        body: body ? JSON.stringify(body) : undefined,
      });
      if (!resp.ok && resp.status !== 204) {
        const text = await resp.text();
        append(`request ${path} failed: ${resp.status} ${text}`);
      }
    } catch (e) {
      append(`request ${path} error: ${e}`);
    }
  }

  function append(line) {
    const log = $("log");
    const ts = new Date().toLocaleTimeString();
    log.textContent += `\n[${ts}] ${line}`;
    log.scrollTop = log.scrollHeight;
  }

  function fmtRelative(unix) {
    if (!unix) return "never";
    const delta = Math.max(0, Math.floor(Date.now() / 1000) - unix);
    if (delta < 5) return "just now";
    if (delta < 60) return `${delta}s ago`;
    if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
    return `${Math.floor(delta / 3600)}h ago`;
  }

  $("btn-login").addEventListener("click", () => {
    const token = $("pat").value.trim();
    const gateway = $("gateway").value.trim();
    if (!token) { append("Enter a PAT first."); return; }
    post("/api/login", { token, gateway: gateway || null });
  });
  $("btn-logout").addEventListener("click", () => post("/api/logout"));
  $("btn-sync").addEventListener("click", () => post("/api/sync"));
  $("btn-validate").addEventListener("click", () => post("/api/validate"));
  $("btn-open-folder").addEventListener("click", () => post("/api/open_folder"));
  $("btn-recheck").addEventListener("click", () => post("/api/probe"));

  function renderServer(snap) {
    const status = snap.gateway_status || { state: "unknown" };
    const dot = $("server-dot");
    const text = $("server-state-text");
    dot.classList.remove("dot-unknown", "dot-probing", "dot-ok", "dot-err");
    let label = "unknown";
    switch (status.state) {
      case "reachable":
        dot.classList.add("dot-ok");
        label = `reachable · ${status.latency_ms}ms`;
        break;
      case "probing":
        dot.classList.add("dot-probing");
        label = "probing…";
        break;
      case "unreachable":
        dot.classList.add("dot-err");
        label = `unreachable · ${status.reason || "unknown error"}`;
        break;
      default:
        dot.classList.add("dot-unknown");
        label = "unknown";
    }
    text.textContent = label;
    $("server-endpoint").textContent = snap.gateway_url || "—";
    $("server-endpoint").classList.toggle("muted", !snap.gateway_url);
    $("server-probe").textContent = fmtRelative(snap.last_probe_at_unix);
    $("server-probe").classList.toggle("muted", !snap.last_probe_at_unix);
  }

  function renderIdentity(snap) {
    const reachable = snap.gateway_status && snap.gateway_status.state === "reachable";
    const id = snap.verified_identity;
    const idEl = $("identity");
    if (!reachable) {
      idEl.textContent = "(gateway unreachable)";
      idEl.classList.add("muted");
    } else if (id && (id.email || id.user_id)) {
      idEl.textContent = id.email || id.user_id;
      idEl.classList.remove("muted");
    } else if (snap.pat_present) {
      idEl.textContent = "(verifying credentials…)";
      idEl.classList.add("muted");
    } else {
      idEl.textContent = "(not signed in)";
      idEl.classList.add("muted");
    }
    $("identity-user").textContent = id && id.user_id ? id.user_id : "—";
    $("identity-user").classList.toggle("muted", !(id && id.user_id));
    $("identity-tenant").textContent = id && id.tenant_id ? id.tenant_id : "—";
    $("identity-tenant").classList.toggle("muted", !(id && id.tenant_id));
  }

  function renderMarketplace(snap) {
    const badge = $("marketplace-status");
    badge.classList.remove("badge-muted", "badge-ok", "badge-warn", "badge-err");
    if (!snap.signed_in) {
      badge.textContent = "sign-in required";
      badge.classList.add("badge-warn");
    } else if (snap.sync_in_flight) {
      badge.textContent = "syncing";
      badge.classList.add("badge-warn");
    } else if (snap.last_sync_summary) {
      badge.textContent = "synced";
      badge.classList.add("badge-ok");
    } else {
      badge.textContent = "never synced";
      badge.classList.add("badge-muted");
    }
  }

  function applySnapshot(snap) {
    renderServer(snap);
    renderIdentity(snap);
    renderMarketplace(snap);

    if (document.activeElement !== $("gateway")) $("gateway").value = snap.gateway_url || "";
    $("plugins-dir").textContent = snap.plugins_dir || "—";
    $("last-sync").textContent = snap.last_sync_summary || "never";
    $("last-sync").classList.toggle("muted", !snap.last_sync_summary);
    $("card-plugins").textContent = snap.plugin_count ?? "—";
    $("card-skills").textContent  = snap.skill_count ?? "—";
    $("card-agents").textContent  = snap.agent_count ?? "—";
    $("card-mcp").textContent     = snap.mcp_count ?? "—";
    $("config-path").textContent = snap.config_file || "";

    const tokenState = snap.cached_token
      ? `cached JWT • ${snap.cached_token.length} bytes • ttl ${snap.cached_token.ttl_seconds}s`
      : (snap.pat_present ? "PAT stored — JWT will refresh on next probe" : "no token");
    $("token-state").textContent = tokenState;
    $("token-state").classList.toggle("muted", !snap.cached_token);

    const pill = $("sync-pill");
    pill.classList.remove("pill-idle", "pill-running", "pill-ok", "pill-err");
    if (snap.sync_in_flight) {
      pill.textContent = "syncing";
      pill.classList.add("pill-running");
    } else if (snap.signed_in) {
      pill.textContent = "ready";
      pill.classList.add("pill-ok");
    } else if (snap.gateway_status && snap.gateway_status.state === "unreachable") {
      pill.textContent = "offline";
      pill.classList.add("pill-err");
    } else {
      pill.textContent = "needs sign-in";
      pill.classList.add("pill-idle");
    }

    $("btn-sync").disabled = !!snap.sync_in_flight || !snap.signed_in;
    $("btn-login").disabled = !!snap.sync_in_flight;
  }

  async function pollState() {
    try {
      const resp = await api("/api/state");
      if (resp.ok) {
        const snap = await resp.json();
        applySnapshot(snap);
      }
    } catch (e) {
      console.error("state poll failed", e);
    } finally {
      setTimeout(pollState, STATE_POLL_MS);
    }
  }

  async function pollLog() {
    try {
      const resp = await api(`/api/log?since=${logCursor}`);
      if (resp.ok) {
        const entries = await resp.json();
        for (const e of entries) {
          append(e.line);
          if (e.id > logCursor) logCursor = e.id;
        }
      }
    } catch (e) {
      console.error("log poll failed", e);
    } finally {
      setTimeout(pollLog, LOG_POLL_MS);
    }
  }

  pollState();
  pollLog();
})();
