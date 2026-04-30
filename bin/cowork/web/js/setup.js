import { $ } from "./dom.js?t=__TOKEN__";
import { post } from "./api.js?t=__TOKEN__";
import { append } from "./activity.js?t=__TOKEN__";

let gatewayDebounceTimer = null;
let lastSavedGateway = "";

export function setSetupError(msg) {
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

export function getLastSavedGateway() { return lastSavedGateway; }
export function setLastSavedGateway(v) { lastSavedGateway = v; }

export function updateSetupPatLink() {
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

function scheduleGatewayPersist() {
  clearTimeout(gatewayDebounceTimer);
  gatewayDebounceTimer = setTimeout(() => {
    const url = $("setup-gateway").value.trim();
    if (!url || url === lastSavedGateway) return;
    lastSavedGateway = url;
    post("/api/gateway", { url }, append);
  }, 600);
}

export function initSetup() {
  $("setup-gateway").addEventListener("input", () => {
    updateSetupPatLink();
    scheduleGatewayPersist();
  });
  $("setup-gateway").addEventListener("blur", () => {
    clearTimeout(gatewayDebounceTimer);
    const url = $("setup-gateway").value.trim();
    if (url && url !== lastSavedGateway) {
      lastSavedGateway = url;
      post("/api/gateway", { url }, append);
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
      post("/api/probe", null, append);
      return;
    }
    const token = input.value.trim();
    if (!token) { setSetupError("Paste your personal access token."); return; }
    setSetupError("");
    lastSavedGateway = gateway;
    post("/api/login", { token, gateway }, append);
  });

  const skip = $("setup-skip-agents");
  if (skip) skip.addEventListener("click", () => {
    post("/api/setup/complete", null, append);
    setSetupStep("done");
  });
  const finish = $("setup-finish");
  if (finish) finish.addEventListener("click", () => {
    post("/api/setup/complete", null, append);
    setSetupStep("done");
  });
  const open = $("setup-open");
  if (open) open.addEventListener("click", () => {
    document.body.classList.remove("setup-mode");
  });
}

export function setSetupStep(step) {
  document.body.dataset.setupStep = step;
  const label = $("setup-step-label");
  if (label) {
    const map = { connect: "Step 1 of 3", agents: "Step 2 of 3", done: "Step 3 of 3" };
    label.textContent = map[step] || "";
  }
}

export function renderSetupAgents(snap) {
  const list = $("setup-agents-list");
  if (!list) return;
  const hosts = snap.host_apps || [];
  list.replaceChildren();
  if (hosts.length === 0) {
    const empty = document.createElement("div");
    empty.className = "muted";
    empty.textContent = "No agents available on this platform.";
    list.appendChild(empty);
    return;
  }
  for (const host of hosts) {
    const row = document.createElement("div");
    row.className = "setup-agent-row";
    const installed = host.snapshot?.profile_state?.kind === "installed";
    row.dataset.state = installed ? "installed" : "absent";
    const meta = document.createElement("div");
    meta.className = "setup-agent-meta";
    const name = document.createElement("div");
    name.className = "setup-agent-name";
    name.textContent = host.display_name + (host.kind === "cli_tool" ? " · CLI" : " · Desktop");
    const desc = document.createElement("div");
    desc.className = "setup-agent-desc";
    desc.textContent = host.description || "";
    meta.append(name, desc);
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = installed ? "ghost" : "primary";
    btn.textContent = installed ? "Installed ✓" : "Install profile";
    btn.disabled = installed;
    btn.addEventListener("click", () => {
      post(`/api/hosts/${encodeURIComponent(host.id)}/profile/generate`, null, append);
    });
    row.append(meta, btn);
    list.appendChild(row);
  }
}
