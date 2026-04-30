import { $ } from "../dom.js?t=__TOKEN__";

const createEmpty = () => {
  const empty = document.createElement("div");
  empty.className = "muted";
  empty.textContent = "No agents available on this platform.";
  return empty;
};

const createMeta = (host) => {
  const meta = document.createElement("div");
  meta.className = "setup-agent-meta";
  const name = document.createElement("div");
  name.className = "setup-agent-name";
  const suffix = host.kind === "cli_tool" ? " · CLI" : " · Desktop";
  name.textContent = host.display_name + suffix;
  const desc = document.createElement("div");
  desc.className = "setup-agent-desc";
  desc.textContent = host.description || "";
  meta.append(name, desc);
  return meta;
};

const createButton = (host, installed) => {
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = installed ? "ghost" : "primary";
  btn.textContent = installed ? "Installed ✓" : "Install profile";
  btn.disabled = installed;
  btn.dataset.action = "host-generate";
  btn.dataset.host = host.id;
  return btn;
};

const createRow = (host) => {
  const row = document.createElement("div");
  row.className = "setup-agent-row";
  const installed = host.snapshot?.profile_state?.kind === "installed";
  row.dataset.state = installed ? "installed" : "absent";
  row.append(createMeta(host), createButton(host, installed));
  return row;
};

export const renderSetupAgents = (snap) => {
  const list = $("setup-agents-list");
  if (!list) {
    return;
  }
  const hosts = snap.host_apps || [];
  list.replaceChildren();
  if (hosts.length === 0) {
    list.append(createEmpty());
    return;
  }
  for (const host of hosts) {
    list.append(createRow(host));
  }
};
