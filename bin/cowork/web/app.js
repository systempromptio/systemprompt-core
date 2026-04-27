(function () {
  const $ = (id) => document.getElementById(id);

  function send(payload) {
    try {
      window.ipc.postMessage(JSON.stringify(payload));
    } catch (e) {
      console.error("ipc post failed", e);
    }
  }

  function append(line) {
    const log = $("log");
    const ts = new Date().toLocaleTimeString();
    log.textContent += `\n[${ts}] ${line}`;
    log.scrollTop = log.scrollHeight;
  }

  $("btn-login").addEventListener("click", () => {
    const token = $("pat").value.trim();
    const gateway = $("gateway").value.trim();
    if (!token) { append("Enter a PAT first."); return; }
    send({ action: "login", token, gateway: gateway || null });
  });
  $("btn-logout").addEventListener("click", () => send({ action: "logout" }));
  $("btn-sync").addEventListener("click", () => send({ action: "sync" }));
  $("btn-validate").addEventListener("click", () => send({ action: "validate" }));
  $("btn-open-folder").addEventListener("click", () => send({ action: "open_folder" }));

  window.systemprompt = {
    update(snap) {
      $("identity").textContent = snap.identity || "(not signed in)";
      $("identity").classList.toggle("muted", !snap.identity);
      if (document.activeElement !== $("gateway")) $("gateway").value = snap.gateway_url || "";
      $("plugins-dir").textContent = snap.plugins_dir || "—";
      $("last-sync").textContent = snap.last_sync_summary || "never";
      $("last-sync").classList.toggle("muted", !snap.last_sync_summary);
      $("card-skills").textContent = snap.skill_count ?? "—";
      $("card-agents").textContent = snap.agent_count ?? "—";
      $("card-mcp").textContent    = snap.mcp_count ?? "—";
      $("config-path").textContent = snap.config_file || "";

      const tokenState = snap.cached_token
        ? `cached JWT • ${snap.cached_token.length} bytes • ttl ${snap.cached_token.ttl_seconds}s`
        : (snap.pat_present ? "PAT stored — JWT will refresh on next run" : "no token");
      $("token-state").textContent = tokenState;
      $("token-state").classList.toggle("muted", !snap.cached_token);

      const pill = $("sync-pill");
      pill.classList.remove("pill-idle", "pill-running", "pill-ok", "pill-err");
      if (snap.sync_in_flight) {
        pill.textContent = "syncing";
        pill.classList.add("pill-running");
      } else {
        pill.textContent = snap.identity ? "ready" : "needs sign-in";
        pill.classList.add("pill-idle");
      }

      const btn = $("btn-sync");
      btn.disabled = !!snap.sync_in_flight;
    },
    log(line) { append(line); },
  };

  send({ action: "ready" });
})();
