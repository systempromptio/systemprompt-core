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
    if (drawerCollapsed) {
      unreadActivity++;
      renderActivityBadge();
    }
  }

  let drawerCollapsed = false;
  let unreadActivity = 0;
  function renderActivityBadge() {
    const badge = $("activity-badge");
    if (!badge) return;
    if (unreadActivity > 0 && drawerCollapsed) {
      badge.textContent = String(unreadActivity);
      badge.hidden = false;
    } else {
      badge.hidden = true;
    }
  }
  function setDrawerCollapsed(next) {
    drawerCollapsed = !!next;
    const drawer = $("activity-drawer");
    if (drawer) drawer.dataset.collapsed = drawerCollapsed ? "true" : "false";
    const btn = $("activity-toggle");
    if (btn) btn.textContent = drawerCollapsed ? "◀" : "▶";
    if (!drawerCollapsed) { unreadActivity = 0; renderActivityBadge(); }
    try { localStorage.setItem("cowork.drawer", drawerCollapsed ? "1" : "0"); } catch (_) {}
  }
  function activateTab(name) {
    for (const btn of document.querySelectorAll(".rail-tab")) {
      btn.setAttribute("aria-selected", btn.dataset.tab === name ? "true" : "false");
    }
    for (const panel of document.querySelectorAll(".tab-panel")) {
      panel.hidden = panel.dataset.tab !== name;
    }
    try { localStorage.setItem("cowork.tab", name); } catch (_) {}
  }
  for (const btn of document.querySelectorAll(".rail-tab")) {
    btn.addEventListener("click", () => activateTab(btn.dataset.tab));
  }
  const initialTab = (() => {
    try { return localStorage.getItem("cowork.tab") || "marketplace"; } catch (_) { return "marketplace"; }
  })();
  activateTab(initialTab);
  const drawerToggleHandler = () => setDrawerCollapsed(!drawerCollapsed);
  $("activity-toggle")?.addEventListener("click", drawerToggleHandler);
  $("activity-toggle-top")?.addEventListener("click", drawerToggleHandler);
  try {
    if (localStorage.getItem("cowork.drawer") === "1") setDrawerCollapsed(true);
  } catch (_) {}

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

  $("setup-pat").addEventListener("focus", () => {
    const input = $("setup-pat");
    if (input.dataset.saved === "1") {
      input.value = "";
      delete input.dataset.saved;
    }
  });

  $("setup-connect").addEventListener("click", () => {
    const input = $("setup-pat");
    const gateway = $("setup-gateway").value.trim();
    if (!gateway) { setSetupError("Enter the gateway URL."); return; }
    if (input.dataset.saved === "1") {
      setSetupError("");
      lastSavedGateway = gateway;
      post("/api/probe");
      return;
    }
    const token = input.value.trim();
    if (!token) { setSetupError("Paste your personal access token."); return; }
    setSetupError("");
    lastSavedGateway = gateway;
    post("/api/login", { token, gateway });
  });

  $("btn-logout").addEventListener("click", () => post("/api/logout"));
  $("btn-sync").addEventListener("click", () => post("/api/sync"));
  $("btn-validate").addEventListener("click", () => post("/api/validate"));
  $("btn-open-folder").addEventListener("click", () => post("/api/open_folder"));
  $("btn-recheck").addEventListener("click", () => post("/api/probe"));
  $("btn-settings-folder")?.addEventListener("click", () => post("/api/open_folder"));
  $("btn-settings-validate")?.addEventListener("click", () => post("/api/validate"));
  $("btn-settings-change-gateway")?.addEventListener("click", () => {
    document.body.classList.add("setup-mode");
  });

  let lastSnapshot = null;
  const hostCards = new Map();

  function setDot(el, cls) {
    if (!el) return;
    el.classList.remove("dot-unknown", "dot-probing", "dot-ok", "dot-err", "dot-warn");
    el.classList.add(cls);
  }

  function setBadge(el, text, cls) {
    if (!el) return;
    el.textContent = text;
    el.classList.remove("badge-muted", "badge-ok", "badge-warn", "badge-err");
    el.classList.add(cls);
  }

  function renderCloud(snap) {
    const status = snap.gateway_status || { state: "unknown" };
    const dot = $("server-dot");
    const text = $("server-state-text");
    let label = "unknown";
    let cls = "dot-unknown";
    switch (status.state) {
      case "reachable": cls = "dot-ok";      label = `reachable · ${status.latency_ms}ms`; break;
      case "probing":   cls = "dot-probing"; label = "probing…"; break;
      case "unreachable": cls = "dot-err";   label = `unreachable · ${status.reason || "unknown error"}`; break;
      default:          cls = "dot-unknown"; label = "unknown";
    }
    setDot(dot, cls);
    text.textContent = label;
    $("server-endpoint").textContent = snap.gateway_url || "—";
    $("server-endpoint").classList.toggle("muted", !snap.gateway_url);
    $("server-probe").textContent = fmtRelative(snap.last_probe_at_unix);
    $("server-probe").classList.toggle("muted", !snap.last_probe_at_unix);

    const reachable = status.state === "reachable";
    const id = snap.verified_identity;
    const idEl = $("identity");
    const idDot = $("identity-dot");
    if (!reachable) {
      setDot(idDot, "dot-unknown");
      idEl.textContent = "(gateway unreachable)";
      idEl.classList.add("muted");
    } else if (id && (id.email || id.user_id)) {
      setDot(idDot, "dot-ok");
      idEl.textContent = id.email || id.user_id;
      idEl.classList.remove("muted");
    } else if (snap.pat_present) {
      setDot(idDot, "dot-probing");
      idEl.textContent = "(verifying credentials…)";
      idEl.classList.add("muted");
    } else {
      setDot(idDot, "dot-warn");
      idEl.textContent = "(not signed in)";
      idEl.classList.add("muted");
    }
    $("identity-user").textContent = id && id.user_id ? id.user_id : "—";
    $("identity-tenant").textContent = id && id.tenant_id ? id.tenant_id : "—";

    const tokenState = snap.cached_token
      ? `cached JWT • ${snap.cached_token.length} bytes • ttl ${snap.cached_token.ttl_seconds}s`
      : (snap.pat_present ? "PAT stored — JWT will refresh on next probe" : "no token");
    $("token-state").textContent = tokenState;
  }

  function renderProxy(snap) {
    const proxy = snap.local_proxy || { state: "Unknown" };
    const state = (proxy.state || "Unknown").toString();
    const dot = $("proxy-dot");
    const text = $("proxy-text");
    let cls = "dot-unknown";
    let label = state;
    switch (state) {
      case "Listening":    cls = "dot-ok";   label = `listening · ${proxy.latency_ms ?? "?"}ms`; break;
      case "Refused":      cls = "dot-err";  label = "connection refused"; break;
      case "Timeout":      cls = "dot-err";  label = "timed out"; break;
      case "HttpError":    cls = "dot-err";  label = `error: ${proxy.error || "unknown"}`; break;
      case "Unconfigured": cls = "dot-warn"; label = "awaiting first host-app probe"; break;
      default:             cls = "dot-unknown"; label = "checking…";
    }
    setDot(dot, cls);
    text.textContent = label;
    $("proxy-detail").textContent = proxy.url || "(no proxy URL configured yet)";
    $("proxy-detail").classList.toggle("muted", !proxy.url);

    const models = collectInferenceModels(snap);
    const epDot = $("endpoints-dot");
    const epText = $("endpoints-text");
    if (models.length === 0) {
      setDot(epDot, "dot-unknown");
      epText.textContent = "no models configured yet";
      epText.classList.add("muted");
    } else {
      setDot(epDot, "dot-ok");
      epText.textContent = models.join(", ");
      epText.classList.remove("muted");
    }
  }

  function collectInferenceModels(snap) {
    const seen = new Set();
    const out = [];
    for (const host of (snap.host_apps || [])) {
      const raw = host.snapshot?.profile_keys?.inferenceModels;
      if (!raw) continue;
      for (const m of raw.split(",")) {
        const t = m.trim();
        if (t && !seen.has(t)) { seen.add(t); out.push(t); }
      }
    }
    return out;
  }

  function ensureHostCard(host) {
    let card = hostCards.get(host.id);
    if (card) return card;
    const tmpl = $("host-card-template");
    const node = tmpl.content.firstElementChild.cloneNode(true);
    node.dataset.hostId = host.id;
    const refs = {
      root: node,
      name: node.querySelector(".host-card-name"),
      badge: node.querySelector('[data-role="badge"]'),
      profileDot: node.querySelector('[data-role="profile-dot"]'),
      profileText: node.querySelector('[data-role="profile-text"]'),
      profileDetail: node.querySelector('[data-role="profile-detail"]'),
      runningDot: node.querySelector('[data-role="running-dot"]'),
      runningText: node.querySelector('[data-role="running-text"]'),
      runningDetail: node.querySelector('[data-role="running-detail"]'),
      btnGenerate: node.querySelector('[data-role="generate"]'),
      btnInstall: node.querySelector('[data-role="install"]'),
      btnReverify: node.querySelector('[data-role="reverify"]'),
      prefs: node.querySelector('[data-role="prefs"]'),
      jwtWarn: node.querySelector('[data-role="jwt-warn"]'),
    };
    refs.btnGenerate.addEventListener("click", () =>
      post(`/api/hosts/${encodeURIComponent(host.id)}/profile/generate`));
    refs.btnReverify.addEventListener("click", () =>
      post(`/api/hosts/${encodeURIComponent(host.id)}/probe`));
    refs.btnInstall.addEventListener("click", () => {
      const path = refs.btnInstall.dataset.path;
      if (!path) { append(`[${host.id}] No generated profile yet — click Generate first.`); return; }
      post(`/api/hosts/${encodeURIComponent(host.id)}/profile/install`, { path });
    });
    $("hosts-list").appendChild(node);
    hostCards.set(host.id, refs);
    return refs;
  }

  function renderHostCard(host, snap) {
    const refs = ensureHostCard(host);
    refs.name.textContent = host.display_name;
    if (host.last_generated_profile) {
      refs.btnInstall.disabled = false;
      refs.btnInstall.dataset.path = host.last_generated_profile;
      refs.btnInstall.title = host.last_generated_profile;
    } else {
      refs.btnInstall.disabled = true;
      delete refs.btnInstall.dataset.path;
      refs.btnInstall.title = "Generate first";
    }

    const hs = host.snapshot;
    if (!hs) {
      setBadge(refs.badge, "probing…", "badge-muted");
      return;
    }

    const profileState = hs.profile_state || { kind: "absent" };
    const missing = profileState.missing_required || [];
    const installed = profileState.kind === "installed";
    const partial = profileState.kind === "partial";
    if (installed) {
      setDot(refs.profileDot, "dot-ok");
      refs.profileText.textContent = "installed";
    } else if (partial) {
      setDot(refs.profileDot, "dot-warn");
      refs.profileText.textContent = `partial (missing: ${missing.join(", ")})`;
    } else {
      setDot(refs.profileDot, "dot-err");
      refs.profileText.textContent = "not installed";
    }
    refs.profileDetail.textContent = hs.profile_source || "—";
    refs.profileDetail.classList.toggle("muted", !hs.profile_source);

    if (hs.host_running) {
      setDot(refs.runningDot, "dot-ok");
      refs.runningText.textContent = "running";
      refs.runningDetail.textContent = (hs.host_processes || []).join(", ") || "process detected";
      refs.runningDetail.classList.remove("muted");
    } else {
      setDot(refs.runningDot, "dot-warn");
      refs.runningText.textContent = "not running";
      refs.runningDetail.textContent = "launch the app to verify routing";
      refs.runningDetail.classList.add("muted");
    }

    const proxyState = (snap.local_proxy?.state || "Unknown").toString();
    let badgeText, badgeCls;
    if (!installed) {
      badgeText = "profile not installed";
      badgeCls = "badge-warn";
    } else if (partial) {
      badgeText = "partial";
      badgeCls = "badge-warn";
    } else if (proxyState === "Unconfigured") {
      badgeText = "awaiting first launch";
      badgeCls = "badge-warn";
    } else if (proxyState === "Listening") {
      badgeText = "healthy";
      badgeCls = "badge-ok";
    } else {
      badgeText = "local proxy down";
      badgeCls = "badge-err";
    }
    setBadge(refs.badge, badgeText, badgeCls);

    const lines = [];
    const keys = hs.profile_keys || {};
    if (Object.keys(keys).length === 0) {
      lines.push("(no keys present)");
    } else {
      for (const [k, v] of Object.entries(keys)) lines.push(`${k} = ${v}`);
    }
    refs.prefs.textContent = lines.join("\n");

    if (snap.cached_token && snap.cached_token.ttl_seconds < 600 && installed) {
      refs.jwtWarn.hidden = false;
      refs.jwtWarn.textContent = `JWT in profile expires in ~${snap.cached_token.ttl_seconds}s — re-generate before it lapses.`;
    } else {
      refs.jwtWarn.hidden = true;
      refs.jwtWarn.textContent = "";
    }
  }

  function renderHosts(snap) {
    const list = snap.host_apps || [];
    const presentIds = new Set(list.map((h) => h.id));
    for (const [id, refs] of hostCards.entries()) {
      if (!presentIds.has(id)) {
        refs.root.remove();
        hostCards.delete(id);
      }
    }
    if (list.length === 0) {
      const placeholder = $("hosts-list");
      if (placeholder && placeholder.children.length === 0) {
        placeholder.innerHTML = '<div class="muted" style="padding:14px 18px;">No host apps registered on this platform.</div>';
      }
      return;
    } else {
      const placeholder = $("hosts-list");
      const noHostsMsg = placeholder?.querySelector(":scope > .muted");
      if (noHostsMsg) noHostsMsg.remove();
    }
    for (const host of list) {
      renderHostCard(host, snap);
    }
  }

  function renderOverallBadge(snap) {
    const cloudState = (snap.gateway_status?.state || "unknown");
    if (cloudState === "probing" || cloudState === "unknown") {
      setBadge($("overall-badge"), "checking…", "badge-muted");
      return;
    }
    if (cloudState === "unreachable") {
      setBadge($("overall-badge"), "cloud unreachable", "badge-err");
      return;
    }
    const hosts = snap.host_apps || [];
    if (hosts.length === 0) {
      setBadge($("overall-badge"), "no host apps", "badge-muted");
      return;
    }
    const proxyState = (snap.local_proxy?.state || "Unknown").toString();
    const anyAbsent = hosts.some((h) => (h.snapshot?.profile_state?.kind || "absent") === "absent");
    const anyPartial = hosts.some((h) => h.snapshot?.profile_state?.kind === "partial");
    const allInstalled = hosts.every((h) => h.snapshot?.profile_state?.kind === "installed");
    if (anyAbsent) { setBadge($("overall-badge"), "profile not installed", "badge-warn"); return; }
    if (anyPartial) { setBadge($("overall-badge"), "profile partial", "badge-warn"); return; }
    if (allInstalled && proxyState === "Unconfigured") {
      setBadge($("overall-badge"), "awaiting first launch", "badge-warn"); return;
    }
    if (allInstalled && proxyState === "Listening") {
      setBadge($("overall-badge"), "healthy", "badge-ok"); return;
    }
    if (allInstalled) {
      setBadge($("overall-badge"), "local proxy down", "badge-err"); return;
    }
    setBadge($("overall-badge"), "checking…", "badge-muted");
  }

  function renderMarketplaceBadge(snap) {
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
    lastSnapshot = snap;
    renderCloud(snap);
    renderProxy(snap);
    renderHosts(snap);
    renderOverallBadge(snap);
    renderMarketplaceBadge(snap);

    applySetupMode(snap);
    $("plugins-dir").textContent = snap.plugins_dir || "—";
    $("last-sync").textContent = snap.last_sync_summary || "never";
    $("last-sync").classList.toggle("muted", !snap.last_sync_summary);
    $("card-plugins").textContent = snap.plugin_count ?? "—";
    $("card-skills").textContent  = snap.skill_count ?? "—";
    $("card-agents").textContent  = snap.agent_count ?? "—";
    $("card-mcp").textContent     = snap.mcp_count ?? "—";
    if ($("card-hooks")) $("card-hooks").textContent = snap.hook_count ?? "0";
    const totals = [snap.plugin_count, snap.skill_count, snap.agent_count, snap.mcp_count, snap.hook_count]
      .filter((n) => typeof n === "number")
      .reduce((a, b) => a + b, 0);
    if ($("rail-count-marketplace")) $("rail-count-marketplace").textContent = String(totals || 0);
    maybeRefreshMarketplace(snap);
    $("config-path").textContent = snap.config_file || "";
    if ($("settings-gateway"))      { $("settings-gateway").textContent      = snap.gateway_url || "—"; $("settings-gateway").classList.toggle("muted", !snap.gateway_url); }
    if ($("settings-plugins-dir"))  { $("settings-plugins-dir").textContent  = snap.plugins_dir || "—"; $("settings-plugins-dir").classList.toggle("muted", !snap.plugins_dir); }
    if ($("settings-config"))       { $("settings-config").textContent       = snap.config_file || "—"; $("settings-config").classList.toggle("muted", !snap.config_file); }

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
    const patInput = $("setup-pat");
    if (snap.pat_present && document.activeElement !== patInput && patInput.dataset.saved !== "1" && patInput.value === "") {
      patInput.value = "•".repeat(24);
      patInput.dataset.saved = "1";
    } else if (!snap.pat_present && patInput.dataset.saved === "1") {
      patInput.value = "";
      delete patInput.dataset.saved;
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

  let mktData = null;
  let mktKind = "plugins";
  let mktSelectedId = null;
  let mktSearch = "";
  let mktLastSyncSummary = null;
  let mktInFlight = false;

  function maybeRefreshMarketplace(snap) {
    if (!snap.signed_in) return;
    if (snap.last_sync_summary === mktLastSyncSummary && mktData) return;
    mktLastSyncSummary = snap.last_sync_summary;
    fetchMarketplace();
  }

  async function fetchMarketplace() {
    if (mktInFlight) return;
    mktInFlight = true;
    try {
      const resp = await api("/api/marketplace");
      if (resp.ok) {
        mktData = await resp.json();
        renderMarketplace();
      }
    } catch (e) {
      console.error("marketplace fetch failed", e);
    } finally {
      mktInFlight = false;
    }
  }

  function renderMarketplace() {
    if (!mktData) return;
    const counts = {
      plugins: (mktData.plugins || []).length,
      skills:  (mktData.skills  || []).length,
      hooks:   (mktData.hooks   || []).length,
      mcp:     (mktData.mcp     || []).length,
      agents:  (mktData.agents  || []).length,
    };
    for (const [k, n] of Object.entries(counts)) {
      const el = document.querySelector(`.mkt-cat[data-kind="${k}"] .mkt-cat-count`);
      if (el) el.textContent = String(n);
    }
    const list = $("mkt-items");
    if (!list) return;
    const items = (mktData[mktKind] || []).filter((it) => {
      if (!mktSearch) return true;
      const q = mktSearch.toLowerCase();
      return (it.name || "").toLowerCase().includes(q) ||
             (it.id   || "").toLowerCase().includes(q) ||
             (it.summary || "").toLowerCase().includes(q);
    });
    list.innerHTML = "";
    if (items.length === 0) {
      const li = document.createElement("li");
      li.className = "mkt-empty-state";
      li.textContent = mktSearch ? "No matches." : "Nothing here yet — sync to populate.";
      list.appendChild(li);
    } else {
      for (const it of items) {
        const li = document.createElement("li");
        li.className = "mkt-item";
        li.dataset.id = it.id;
        li.setAttribute("aria-selected", String(it.id === mktSelectedId));
        const name = document.createElement("div");
        name.className = "mkt-item-name";
        name.textContent = it.name || it.id;
        const meta = document.createElement("div");
        meta.className = "mkt-item-meta";
        meta.textContent = (it.summary || it.source || "").slice(0, 120);
        li.appendChild(name);
        if (meta.textContent) li.appendChild(meta);
        li.addEventListener("click", () => {
          mktSelectedId = it.id;
          renderMarketplace();
        });
        list.appendChild(li);
      }
    }
    renderMarketplaceDetail();
  }

  function renderMarketplaceDetail() {
    const detail = $("mkt-detail");
    if (!detail || !mktData) return;
    const items = mktData[mktKind] || [];
    const selected = items.find((it) => it.id === mktSelectedId) || null;
    detail.innerHTML = "";
    if (!selected) {
      const empty = document.createElement("div");
      empty.className = "mkt-empty";
      empty.textContent = items.length ? "Pick an item to inspect." : "Sync to populate the marketplace.";
      detail.appendChild(empty);
      return;
    }
    const h = document.createElement("h2");
    h.textContent = selected.name || selected.id;
    detail.appendChild(h);
    const metaRow = document.createElement("div");
    metaRow.className = "mkt-detail-meta";
    const kindLabel = document.createElement("span");
    kindLabel.textContent = mktKind;
    metaRow.appendChild(kindLabel);
    const sourceLabel = document.createElement("span");
    sourceLabel.textContent = `· ${selected.source}`;
    metaRow.appendChild(sourceLabel);
    detail.appendChild(metaRow);

    if (selected.summary) {
      const p = document.createElement("p");
      p.textContent = selected.summary;
      detail.appendChild(p);
    }
    if (selected.readme) {
      const sec = document.createElement("section");
      sec.className = "mkt-detail-section";
      const h3 = document.createElement("h3");
      h3.textContent = "README";
      const pre = document.createElement("div");
      pre.className = "mkt-detail-readme";
      pre.textContent = selected.readme;
      sec.appendChild(h3);
      sec.appendChild(pre);
      detail.appendChild(sec);
    }
    const sec = document.createElement("section");
    sec.className = "mkt-detail-section";
    const h3 = document.createElement("h3");
    h3.textContent = "Path";
    const path = document.createElement("div");
    path.className = "mkt-detail-path";
    path.textContent = selected.path;
    sec.appendChild(h3);
    sec.appendChild(path);
    detail.appendChild(sec);
  }

  for (const cat of document.querySelectorAll(".mkt-cat")) {
    cat.addEventListener("click", () => {
      mktKind = cat.dataset.kind;
      mktSelectedId = null;
      for (const c of document.querySelectorAll(".mkt-cat")) {
        c.setAttribute("aria-selected", c === cat ? "true" : "false");
      }
      renderMarketplace();
    });
  }
  $("mkt-search")?.addEventListener("input", (e) => {
    mktSearch = e.target.value || "";
    renderMarketplace();
  });

  pollState();
  pollLog();
})();
