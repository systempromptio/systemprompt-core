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

  let gatewayDebounceTimer = null;
  let lastSavedGateway = "";

  function updateSetupPatLink() {
    const link = $("setup-pat-link");
    if (!link) return;
    const gw = $("setup-gateway").value.trim().replace(/\/+$/, "");
    if (gw) {
      link.href = `${gw}/admin/login`;
      link.removeAttribute("aria-disabled");
      link.classList.remove("disabled");
    } else {
      link.href = "#";
      link.setAttribute("aria-disabled", "true");
      link.classList.add("disabled");
    }
  }

  function setSetupError(msg) {
    const el = $("setup-error");
    if (!el) return;
    if (msg) {
      el.textContent = msg;
      el.hidden = false;
    } else {
      el.textContent = "";
      el.hidden = true;
    }
  }

  function scheduleGatewayPersist() {
    clearTimeout(gatewayDebounceTimer);
    gatewayDebounceTimer = setTimeout(() => {
      const url = $("setup-gateway").value.trim();
      if (!url || url === lastSavedGateway) return;
      lastSavedGateway = url;
      post("/api/gateway", { url });
    }, 600);
  }

  $("setup-gateway").addEventListener("input", () => {
    updateSetupPatLink();
    scheduleGatewayPersist();
  });
  $("setup-gateway").addEventListener("blur", () => {
    clearTimeout(gatewayDebounceTimer);
    const url = $("setup-gateway").value.trim();
    if (url && url !== lastSavedGateway) {
      lastSavedGateway = url;
      post("/api/gateway", { url });
    }
  });

  $("setup-pat-link").addEventListener("click", (e) => {
    if ($("setup-pat-link").getAttribute("aria-disabled") === "true") {
      e.preventDefault();
      setSetupError("Enter the gateway URL first.");
    }
  });

  $("setup-connect").addEventListener("click", () => {
    const token = $("setup-pat").value.trim();
    const gateway = $("setup-gateway").value.trim();
    if (!gateway) { setSetupError("Enter the gateway URL."); return; }
    if (!token)   { setSetupError("Paste your personal access token."); return; }
    setSetupError("");
    lastSavedGateway = gateway;
    post("/api/login", { token, gateway });
  });

  $("btn-logout").addEventListener("click", () => post("/api/logout"));
  $("btn-sync").addEventListener("click", () => post("/api/sync"));
  $("btn-validate").addEventListener("click", () => post("/api/validate"));
  $("btn-open-folder").addEventListener("click", () => post("/api/open_folder"));
  $("btn-recheck").addEventListener("click", () => post("/api/probe"));
  $("btn-claude-generate").addEventListener("click", () => post("/api/claude/profile/generate"));
  $("btn-claude-reverify").addEventListener("click", () => post("/api/claude/probe"));
  $("btn-claude-install").addEventListener("click", () => {
    const path = $("btn-claude-install").dataset.path || lastSnapshot?.last_generated_profile;
    if (!path) { append("No generated profile yet — click Generate first."); return; }
    post("/api/claude/profile/install", { path });
  });

  let lastSnapshot = null;

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

  function setDot(id, cls) {
    const el = $(id);
    if (!el) return;
    el.classList.remove("dot-unknown", "dot-probing", "dot-ok", "dot-err", "dot-warn");
    el.classList.add(cls);
  }

  function setBadge(id, text, cls) {
    const el = $(id);
    if (!el) return;
    el.textContent = text;
    el.classList.remove("badge-muted", "badge-ok", "badge-warn", "badge-err");
    el.classList.add(cls);
  }

  function renderInstallButton(snap) {
    const installBtn = $("btn-claude-install");
    if (snap.last_generated_profile) {
      installBtn.disabled = false;
      installBtn.dataset.path = snap.last_generated_profile;
      installBtn.title = snap.last_generated_profile;
    } else {
      installBtn.disabled = true;
      installBtn.removeAttribute("data-path");
      installBtn.title = "Generate first";
    }
  }

  function renderClaude(snap) {
    renderInstallButton(snap);
    const ci = snap.claude_integration;
    if (!ci) {
      setBadge("claude-overall", "probing…", "badge-muted");
      return;
    }

    const desktop = ci.managed_prefs?.desktop;
    const profileInstalled = !!desktop?.installed;
    const missing = desktop?.missing_required || [];
    if (profileInstalled && missing.length === 0) {
      setDot("claude-profile-dot", "dot-ok");
      $("claude-profile-text").textContent = "installed";
      $("claude-profile-detail").textContent = desktop.plist_path || "";
      $("claude-profile-detail").classList.remove("muted");
    } else if (profileInstalled) {
      setDot("claude-profile-dot", "dot-warn");
      $("claude-profile-text").textContent = `partial (missing: ${missing.join(", ")})`;
      $("claude-profile-detail").textContent = desktop.plist_path || "";
      $("claude-profile-detail").classList.remove("muted");
    } else {
      setDot("claude-profile-dot", "dot-err");
      $("claude-profile-text").textContent = "not installed";
      $("claude-profile-detail").textContent = "no managed plist for com.anthropic.claudefordesktop";
      $("claude-profile-detail").classList.add("muted");
    }

    const gh = ci.gateway_health || { state: "Unknown" };
    const gstate = (gh.state || "Unknown").toString();
    if (gstate === "Listening") {
      setDot("claude-gateway-dot", "dot-ok");
      $("claude-gateway-text").textContent = `listening · ${gh.latency_ms ?? "?"}ms`;
    } else if (gstate === "Refused") {
      setDot("claude-gateway-dot", "dot-err");
      $("claude-gateway-text").textContent = "connection refused";
    } else if (gstate === "Timeout") {
      setDot("claude-gateway-dot", "dot-err");
      $("claude-gateway-text").textContent = "timed out";
    } else if (gstate === "Unconfigured") {
      setDot("claude-gateway-dot", "dot-warn");
      $("claude-gateway-text").textContent = "not yet configured";
    } else {
      setDot("claude-gateway-dot", "dot-unknown");
      $("claude-gateway-text").textContent = gstate;
    }
    $("claude-gateway-detail").textContent = gh.url || "";
    $("claude-gateway-detail").classList.toggle("muted", !gh.url);

    if (ci.claude_running) {
      setDot("claude-running-dot", "dot-ok");
      $("claude-running-text").textContent = "running";
      $("claude-running-detail").textContent = (ci.claude_processes || []).join(", ") || "process detected";
      $("claude-running-detail").classList.remove("muted");
    } else {
      setDot("claude-running-dot", "dot-warn");
      $("claude-running-text").textContent = "not running";
      $("claude-running-detail").textContent = "launch Claude.app to verify routing";
      $("claude-running-detail").classList.add("muted");
    }

    const overall = profileInstalled && gstate === "Listening" && missing.length === 0
      ? ["healthy", "badge-ok"]
      : profileInstalled && gstate !== "Listening"
        ? ["gateway down", "badge-err"]
        : !profileInstalled
          ? ["profile missing", "badge-warn"]
          : ["partial", "badge-warn"];
    setBadge("claude-overall", overall[0], overall[1]);

    const prefsLines = [];
    const dKeys = desktop?.keys || {};
    if (Object.keys(dKeys).length === 0) {
      prefsLines.push("(no keys present)");
    } else {
      for (const [k, v] of Object.entries(dKeys)) prefsLines.push(`${k} = ${v}`);
    }
    $("claude-prefs").textContent = prefsLines.join("\n");

    const warn = $("claude-jwt-warn");
    if (snap.cached_token && snap.cached_token.ttl_seconds < 600 && profileInstalled) {
      warn.hidden = false;
      warn.textContent = `JWT in profile expires in ~${snap.cached_token.ttl_seconds}s — re-generate before it lapses.`;
    } else {
      warn.hidden = true;
      warn.textContent = "";
    }
  }

  function applySnapshot(snap) {
    lastSnapshot = snap;
    renderServer(snap);
    renderIdentity(snap);
    renderMarketplace(snap);
    renderClaude(snap);

    applySetupMode(snap);
    $("identity-gateway").textContent = snap.gateway_url || "—";
    $("identity-gateway").classList.toggle("muted", !snap.gateway_url);
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
    $("setup-connect").disabled = !!snap.sync_in_flight;
  }

  function isConfigured(snap) {
    const reachable = snap.gateway_status && snap.gateway_status.state === "reachable";
    const id = snap.verified_identity;
    return !!(reachable && id && id.user_id);
  }

  function applySetupMode(snap) {
    const setup = !isConfigured(snap);
    document.body.classList.toggle("setup-mode", setup);
    if (!setup) {
      setSetupError("");
      return;
    }
    const gwInput = $("setup-gateway");
    if (document.activeElement !== gwInput) {
      const next = snap.gateway_url || "";
      if (gwInput.value !== next) {
        gwInput.value = next;
        lastSavedGateway = next;
      }
      updateSetupPatLink();
    }
    const dot = $("setup-gateway-dot");
    const msg = $("setup-gateway-msg");
    dot.classList.remove("dot-unknown", "dot-probing", "dot-ok", "dot-err");
    const status = snap.gateway_status || { state: "unknown" };
    switch (status.state) {
      case "reachable":
        dot.classList.add("dot-ok");
        msg.textContent = `reachable · ${status.latency_ms}ms`;
        msg.classList.remove("muted");
        break;
      case "probing":
        dot.classList.add("dot-probing");
        msg.textContent = "probing…";
        msg.classList.add("muted");
        break;
      case "unreachable":
        dot.classList.add("dot-err");
        msg.textContent = `unreachable · ${status.reason || "unknown error"}`;
        msg.classList.remove("muted");
        break;
      default:
        dot.classList.add("dot-unknown");
        msg.textContent = snap.gateway_url ? "not yet probed" : "enter a URL to probe…";
        msg.classList.add("muted");
    }
    if (status.state === "reachable" && snap.pat_present && !(snap.verified_identity && snap.verified_identity.user_id)) {
      setSetupError("Token rejected by gateway. Issue a fresh PAT and try again.");
    } else if (status.state === "unreachable" && snap.pat_present) {
      setSetupError(`Gateway unreachable: ${status.reason || "unknown error"}`);
    }
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
