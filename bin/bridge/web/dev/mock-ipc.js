// Stands in for the native webview so the GUI runs in a plain browser.
//
// web/js/bridge.js touches exactly two globals — `window.ipc.postMessage` to
// send, and `window.__bridge.reply` / `.emit` to receive — so a faithful mock is
// small. It answers `state.snapshot` from the fixture named in the URL
// (`?fixture=stale`), and models the write commands well enough that a flow is
// clickable rather than merely a still frame: repairing an agent really does
// move it to installed and re-emit `host.changed`, the way the real bridge does.
//
// Served from disk by `dev-web` only. build.rs excludes web/dev/ from staging,
// so none of this is present in a shipped binary.

const params = new URLSearchParams(location.search);
const fixtureName = params.get("fixture") || "healthy";

function ensureBus() {
  if (!window.__bridge) { window.__bridge = { seq: 0, pending: new Map(), subs: new Map() }; }
  return window.__bridge;
}

function emit(channel, payload) {
  const bus = ensureBus();
  const subs = bus.subs.get(channel);
  if (subs) { for (const cb of subs) { try { cb(payload); } catch (e) { console.error(channel, e); } } }
}

// The real envelope, from src/gui/ipc.rs: `ok` is a boolean and the payload
// travels in `value`. The mock used to resolve with `payload.ok` itself, which
// only worked because it also built the envelope — a mock that disagrees with
// the contract cannot evidence the contract.
function reply(id, payload) {
  const bus = ensureBus();
  const entry = bus.pending.get(id);
  if (!entry) { return; }
  bus.pending.delete(id);
  if (payload && payload.ok) { entry.resolve(payload.value); }
  else { entry.reject(payload && payload.error); }
}

let state = null;
let latencyMs = 0;
let failing = new Set();
let scheduleState = "not_installed";

async function loadFixture() {
  const res = await fetch(`/dev/state?fixture=${encodeURIComponent(fixtureName)}`);
  if (!res.ok) { throw new Error(`fixture "${fixtureName}" not found`); }
  state = rebaseTimestamps(await res.json());
  // Directives are stripped before the snapshot reaches the app: they drive the
  // mock, they are not part of `StatePayload`.
  latencyMs = Number(state.__latencyMs) || 0;
  failing = new Set(Array.isArray(state.__fail) ? state.__fail : []);
  marketplace = state.__marketplace || null;
  // `unknown` is only reachable when the real scheduler declines to answer, so
  // a fixture has to stage it -- the value of the three-way distinction is
  // entirely in what the pane does with the third state.
  scheduleState = state.__schedule || "not_installed";
  delete state.__latencyMs;
  delete state.__fail;
  delete state.__marketplace;
  delete state.__schedule;
  document.title = `bridge preview — ${fixtureName}`;
}

let marketplace = null;

const SKILL_NAMES = [
  "rust-coding-standards", "brand-voice", "cli-usage", "content-publish",
  "extension-building", "identity", "mcp-building", "seo-guide",
  "web-building", "ux-ui-guide", "visual-design-system", "workflow-authoring",
];
const PLUGIN_NAMES = ["commons", "development", "content"];

function item(id, name, summary, path, source) {
  return { id, name, summary, path, source, readme: null, children: [], extra: null };
}

// Derived from the fixture's own counts, so the footer's "12 skills, 3 plugins"
// and the list beside it cannot disagree — the contradiction the review caught
// in its screenshot.
function defaultListing() {
  const skills = SKILL_NAMES.slice(0, state.skill_count || 0).map((n) =>
    item(n, n, `The ${n.replace(/-/g, " ")} skill`, `${state.plugins_dir}\\skills\\${n}`, "org"));
  const plugins = PLUGIN_NAMES.slice(0, state.plugin_count || 0).map((n, i) => ({
    ...item(n, n, `${n} plugin`, `${state.plugins_dir}\\${n}`, "org"),
    version: `1.${i}.0`,
    author: "systemprompt.io",
    homepage: `https://systemprompt.io/plugins/${n}`,
  }));
  const mcp = (state.mcp_auth || []).map((srv) => ({
    ...item(srv.id, srv.display_name || srv.id, null, srv.url || "", "org"),
    extra: {
      proxy_url: `http://127.0.0.1:8899/mcp/${srv.id}`,
      upstream_url: srv.url || "",
      transport: "http",
      tools: srv.tools || [],
    },
  }));
  const agents = Array.from({ length: state.agent_count || 0 }, (_, i) =>
    item(`agent-${i + 1}`, `agent-${i + 1}`, null, `${state.plugins_dir}\\agents\\agent-${i + 1}`, "org"));
  return {
    plugins, skills, agents, hooks: [], mcp, artifacts: [],
    plugins_dir: state.plugins_dir || null,
    last_sync_diff: { installed: [], updated: [], removed: [] },
  };
}

// Why: fixtures carry absolute unix timestamps, so a fixture named `governing`
// silently becomes `no-traffic` once wall-clock passes its stamp -- it stops
// demonstrating the thing it is named for. Every `*_unix` value is shifted by
// one delta, so the intervals the fixture author wrote are the intervals a
// reviewer sees, however long after they wrote them.
//
// Two values are deliberately left alone. `0` is the "never" sentinel --
// `proxy-unreachable` means it has never forwarded anything, and rebasing it
// would read as "just now", the exact inversion of the state. And only
// `*_at_unix` keys set the anchor: `exp_unix` is a future expiry, so letting it
// define "now" would expire the token on arrival.
function eachTimestamp(node, visit) {
  if (Array.isArray(node)) {
    for (const entry of node) { eachTimestamp(entry, visit); }
    return;
  }
  if (!node || typeof node !== "object") { return; }
  for (const [key, value] of Object.entries(node)) {
    if (typeof value === "number") {
      if (key.endsWith("_unix")) { visit(node, key, value); }
    } else {
      eachTimestamp(value, visit);
    }
  }
}

function rebaseTimestamps(s) {
  let anchor = 0;
  eachTimestamp(s, (_owner, key, value) => {
    if (value > 0 && key.endsWith("_at_unix") && value > anchor) { anchor = value; }
  });
  if (!anchor) { return s; }
  const delta = Math.floor(Date.now() / 1000) - anchor;
  eachTimestamp(s, (owner, key, value) => {
    if (value > 0) { owner[key] = value + delta; }
  });
  return s;
}

function hostById(id) {
  return (state.host_apps || []).find((h) => h.id === id) || null;
}

function touch(host) {
  host.snapshot = host.snapshot || {};
  host.snapshot.probed_at_unix = Math.floor(Date.now() / 1000);
  emit("host.changed", host);
  emit("state.changed", state);
}

// Commands that change something change the fixture in memory, so the UI a
// reviewer clicks through behaves like the real one instead of freezing.

const AGENTS = ["claude-desktop", "codex-cli"];

function sampleActivity(limit) {
  const now = Math.floor(Date.now() / 1000);
  const seed = [
    ["info", "local proxy listening on 127.0.0.1:48217"],
    ["info", `settings ui served at ${state.gateway_url || "http://127.0.0.1:8081"}`],
    ["info", state.last_sync_summary ? `sync ok (${state.last_sync_summary})` : "sync never run"],
    ["warn", "proxy: POST /v1/messages \u2192 403 (stale secret; presented_fp=9f2a expected_fp=1c04; secret_path=C:\\ProgramData\\systemprompt\\bridge\\loopback.secret; re-run `bridge install --apply`) [a41f]"],
    ["info", "tokens: +1204 in / +836 out (total 12 msgs)"],
    ["error", "proxy: POST /v1/messages \u2192 error: upstream connection reset by peer [b77c]"],
    ["info", "proxy: GET /v1/models \u2192 200 (18ms) [c019]"],
  ];
  const out = [];
  for (let i = 0; i < 40; i += 1) {
    const [level, line] = seed[i % seed.length];
    out.push({ id: i + 1, ts_unix: now - (40 - i) * 17, level, line });
  }
  return out.slice(-(limit || 500));
}

function sampleRequests(limit) {
  const now = Math.floor(Date.now() / 1000);
  const out = [];
  for (let i = 0; i < 24; i += 1) {
    const denied = i % 11 === 3;
    const failed = i % 7 === 5;
    out.push({
      id: i + 1,
      ts_unix: now - (24 - i) * 23,
      req_id: (0x1000 + i * 37).toString(16),
      agent: AGENTS[i % AGENTS.length],
      method: i % 5 === 0 ? "GET" : "POST",
      path: i % 5 === 0 ? "/v1/models" : "/v1/messages",
      verdict: denied ? "denied" : "forwarded",
      deny_reason: denied ? "secret-mismatch" : null,
      status: denied ? 403 : failed ? 502 : 200,
      latency_ms: denied ? null : 120 + ((i * 37) % 900),
      tokens_in: denied || failed ? null : 900 + i * 31,
      tokens_out: denied || failed ? null : 300 + i * 17,
      cache_read_tokens: denied || failed ? null : i % 3 === 0 ? 2048 : null,
      cache_write_tokens: null,
      model: denied || failed ? null : "claude-opus-4-6-20260401",
      upstream_request_id: denied ? null : `req_${(0x9000 + i).toString(16)}`,
      gateway_decision: denied ? null : i % 9 === 4 ? "deny" : "allow",
      gateway_policy: i % 9 === 4 ? "secret_scan" : "default_allow",
    });
  }
  return out.slice(-(limit || 500));
}

// Mirrors `current()` in src/gui/handlers/settings_write.rs.
// Why: `unknown` is only reachable when the OS scheduler itself declines to
// answer, which no fixture can stage, so the preview takes it from localStorage.
function autostartSeed() {
  try {
    return window.localStorage.getItem("bridge.dev.autostart") || "not_installed";
  } catch (_) {
    return "not_installed";
  }
}

const prefs = {
  autostart: { state: autostartSeed() },
  update_automatic: false,
  session_enabled: false,
};

function settingsPayload() {
  return {
    ...prefs,
    gateway_url: state.gateway_url,
    auth_scheme: "bearer",
    models: ["claude-opus-4", "claude-sonnet-4"],
    cert_keystore_ref: null,
    // Exercises the provenance badge: a device policy has replaced whatever the
    // operator pinned, which the app must say out loud.
    pinned_pubkey: { value: "MCowBQYDK2VwAyEABase64Pubkey==", source: "policy" },
    config_file: state.config_file,
    config_malformed: prefs.config_malformed || state.config_malformed || null,
    schedule: { state: scheduleState, label: "io.systemprompt.bridge.sync" },
  };
}

const COMMANDS = {
  "state.snapshot": () => state,
  // Profile is served by the gateway, not by the state snapshot, so it needs its
  // own reply or the pane sits in its skeleton forever. Shapes mirror
  // `build_profile` (src/gui/handlers/profile.rs) and `BridgeProfileUsage`
  // (crates/shared/models/src/api/cloud/usage.rs).
  //
  // The values are deliberately awkward rather than tidy: a full-precision
  // sub-cent cost, a model id long enough to need truncating, and a mixture of
  // window sizes. Rounded sample data hides exactly the layout faults this pane
  // had.
  "profile.fetch": () => {
    const ident = state.verified_identity || {};
    const hoursAgo = (h) => new Date(Date.now() - h * 3600e3).toISOString();
    return {
      gateway: state.gateway_url || "http://127.0.0.1:8081",
      identity: {
        email: ident.email || "admin@localhost.dev",
        display_name: ident.display_name || "Platform Admin",
        user_id: ident.user_id || "6b6b11f6-0ede-415b-9f5c-e06be78a9724",
        tenant_id: ident.tenant_id || "d24f0c1a-7b53-4a6e-9f10-2c8b41e77a55",
        provider: ident.provider || "odoo",
        roles: ident.roles || ["admin", "user"],
        exp_unix: Math.floor(Date.now() / 1000) + 3600,
      },
      bridge_profile: {
        plan: "enterprise",
        models: [
          "claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5-20251001",
          "claude-sonnet-5-20250929", "claude-opus-5-20260114",
        ],
      },
      usage: {
        d1:  { requests: 41, tokens: 128_400, cost_microdollars: 9_120, previous_cost_microdollars: 7_740 },
        d7:  { requests: 388, tokens: 2_240_900, cost_microdollars: 141_030, previous_cost_microdollars: 168_400 },
        d30: { requests: 1_642, tokens: 9_880_500, cost_microdollars: 612_450, previous_cost_microdollars: 500_120 },
        top_models: [
          { model: "claude-sonnet-5",            requests: 1_190, tokens: 7_012_300, cost_microdollars: 341_900, token_share: 0.7098 },
          { model: "claude-haiku-4-5-20251001",  requests: 302,   tokens: 1_984_100, cost_microdollars: 71_240,  token_share: 0.2008 },
          { model: "claude-opus-5",              requests: 128,   tokens: 812_400,   cost_microdollars: 195_880, token_share: 0.0822 },
          { model: "claude-sonnet-5-20250929",   requests: 22,    tokens: 71_700,    cost_microdollars: 3_430,   token_share: 0.0073 },
        ],
        conversations: {
          total_conversations: 274,
          total_ai_requests: 1_642,
          by_model: [
            { name: "claude-sonnet-5",           conversations: 191, ai_requests: 1_190 },
            { name: "claude-haiku-4-5-20251001", conversations: 58,  ai_requests: 302 },
            { name: "claude-opus-5",             conversations: 21,  ai_requests: 128 },
          ],
          by_agent: [
            { name: "claude-code",  conversations: 203, ai_requests: 1_301 },
            { name: "cursor",       conversations: 44,  ai_requests: 218 },
            { name: "unattributed", conversations: 27,  ai_requests: 123 },
          ],
          recent: [
            { context_id: "9f2c7a41e0b34d58a1c6", last_activity: hoursAgo(0.4), ai_requests: 18, model: "claude-sonnet-5",           agent_name: "claude-code" },
            { context_id: "1a7e33bd90c24f6e88d2", last_activity: hoursAgo(3),   ai_requests: 6,  model: "claude-opus-5",             agent_name: "cursor" },
            { context_id: "c40b8912ff6a4e13b7a0", last_activity: hoursAgo(27),  ai_requests: 41, model: "claude-haiku-4-5-20251001", agent_name: "claude-code" },
            { context_id: "72d5e6c8ab194c079e31", last_activity: hoursAgo(52),  ai_requests: 3,  model: "claude-sonnet-5",           agent_name: null },
          ],
        },
      },
    };
  },
  // A listing, not `{items: []}` — the old reply matched no shape the pane
  // understands, so the Library previewed as permanently empty whatever the
  // fixture said. Shape is `MarketplaceListing` (src/gui/server_marketplace).
  "marketplace.list": () => marketplace || defaultListing(),
  // The Rust ring is seeded from the fixture so the log and the request stream
  // have history on first paint, which is the whole point of the backfill.
  "activity.recent": ({ limit }) => ({ entries: sampleActivity(limit) }),
  "requests.recent": ({ limit }) => ({ entries: sampleRequests(limit) }),
  "host.proxy.probe": () => ({}),
  "diagnostics.info": () => ({
    config_file: state.config_file,
    gateway_url: state.gateway_url,
    plugins_dir: state.plugins_dir,
  }),
  "diagnostics.exportBundle": () => ({ path: "/tmp/preview/bridge-diagnostics.zip" }),
  "openLogFolder": () => ({}),
  "openConfigFolder": () => ({}),
  "openExternalUrl": ({ url }) => { window.open(url, "_blank", "noopener"); return {}; },
  "update.check": () => state.update || {},
  "update.install": () => ({}),
  "update.restart": () => ({}),
  "cancel": () => ({}),
  "quit": () => ({}),
  "mcp.auth.probe": () => ({}),
  "settings.get": () => settingsPayload(),
  "settings.set": ({ key, value }) => {
    const malformed = prefs.config_malformed || state.config_malformed;
    if (key !== "autostart" && malformed) {
      throw new Error(`refusing to save: ${malformed}`);
    }
    prefs[key] = key === "autostart"
      ? { state: value ? "installed" : "not_installed" }
      : value;
    return settingsPayload();
  },
  "validate": () => ({ ok: true }),
  "sync": () => {
    state.sync_in_flight = true;
    emit("state.changed", state);
    setTimeout(() => {
      state.sync_in_flight = false;
      state.last_sync_summary = `${state.skill_count} skills, ${state.plugin_count} plugins`;
      emit("state.changed", state);
    }, Math.max(latencyMs, 600));
    return {};
  },
  "gateway.set": ({ url }) => {
    state.gateway_url = url;
    emit("state.changed", state);
    return {};
  },
  "gateway.probe": () => ({}),
  "session.login": ({ gateway }) => {
    state.gateway_url = gateway || state.gateway_url;
    state.signed_in = true;
    state.verified_identity = state.verified_identity || {
      email: "admin@localhost.dev",
      user_id: "user_1",
      tenant_id: "tenant_1",
      exp_unix: Math.floor(Date.now() / 1000) + 3600,
      verified_at_unix: Math.floor(Date.now() / 1000),
    };
    state.gateway_status = { state: "reachable", latency_ms: 38 };
    emit("state.changed", state);
    return {};
  },
  "login": (args) => COMMANDS["session.login"](args),
  "logout": () => {
    state.signed_in = false;
    state.verified_identity = null;
    state.cached_token = null;
    emit("state.changed", state);
    return {};
  },
  "setup.complete": () => {
    state.agents_onboarded = true;
    emit("state.changed", state);
    return {};
  },
  "agent.open": () => ({}),
  "agent.openConfig": () => ({}),
  "agent.uninstall": ({ hostId }) => {
    const host = hostById(hostId);
    if (!host) { return { removed: false }; }
    // Both hosts hand macOS a configuration profile the user must remove
    // themselves; the reply carries that instruction rather than claiming a
    // removal that did not happen.
    if (host.config_format === "mobileconfig") {
      return { removed: false, instruction: "Remove the profile under System Settings › Device Management." };
    }
    host.snapshot = host.snapshot || {};
    host.snapshot.profile_state = { kind: "absent" };
    host.snapshot.profile_keys = {};
    host.last_generated_profile = null;
    touch(host);
    return { removed: true, path: host.snapshot.profile_source };
  },
  "host.profile.generate": ({ hostId }) => {
    const host = hostById(hostId);
    if (!host) { return {}; }
    host.last_generated_profile = host.last_generated_profile || {
      path: `/tmp/preview/${hostId}-profile.json`,
      bytes: 712,
      profile_uuid: "ce0b78a5-6d14-4cwk-cwk0-feedfaceb43c",
      payload_uuid: "ce0a6a91-3b6c-4cwk-cwk0-deadbeefa06f",
    };
    return { path: host.last_generated_profile.path };
  },
  "host.profile.install": ({ hostId }) => {
    const host = hostById(hostId);
    if (host) {
      host.snapshot = host.snapshot || {};
      host.snapshot.profile_state = { kind: "installed" };
      host.snapshot.app_installed = host.snapshot.app_installed || "installed";
      touch(host);
    }
    return {};
  },
  "host.probe": ({ hostId }) => {
    const host = hostById(hostId);
    if (host) { touch(host); }
    return {};
  },
  "host.model-filter.set": ({ hostId, protocols }) => {
    const host = hostById(hostId);
    if (host) {
      host.model_protocols = protocols || [];
      host.model_protocols_overridden = protocols !== null && protocols !== undefined;
      touch(host);
    }
    return {};
  },
};

const FAIL_SCOPE = {
  sync: "marketplace",
  validate: "marketplace",
  "marketplace.list": "marketplace",
  "gateway.set": "gateway",
  "gateway.probe": "gateway",
  "session.login": "identity",
  login: "identity",
  logout: "identity",
  "profile.fetch": "identity",
};

function failureFor(cmd) {
  return {
    scope: FAIL_SCOPE[cmd] || "host",
    code: cmd === "session.login" || cmd === "login" ? "unauthorized" : "unreachable",
    message: `${cmd} failed: the preview fixture asks this command to fail`,
  };
}

window.ipc = {
  postMessage(raw) {
    const { id, cmd, args } = JSON.parse(raw);
    const handler = COMMANDS[cmd];
    const run = () => {
      if (failing.has(cmd)) {
        const error = failureFor(cmd);
        // The Rust `finish()` emits on the error channel *as well as* rejecting,
        // so a mock that only rejects cannot show the double-toast the real app
        // would produce.
        emit("error", error);
        reply(id, { ok: false, error });
        return;
      }
      if (!handler) {
        console.info(`[mock-ipc] unhandled ${cmd}`, args);
        reply(id, { ok: true, value: {} });
        return;
      }
      reply(id, { ok: true, value: handler(args || {}) });
    };
    // A real IPC round-trip is never synchronous; resolving in a microtask
    // keeps the components on the same code path they take in the app.
    if (latencyMs > 0) { setTimeout(run, latencyMs); } else { queueMicrotask(run); }
  },
};

ensureBus();
await loadFixture();
// The components ask for a snapshot on connect. Anything that mounted before
// the fixture landed gets it here.
emit("state.changed", state);

// A small on-page switcher: the whole point is to compare states quickly.
const bar = document.createElement("nav");
bar.style.cssText = "position:fixed;bottom:12px;left:50%;transform:translateX(-50%);z-index:9999;display:flex;gap:6px;padding:6px 10px;background:oklch(0.16 0.01 45/0.92);border:1px solid oklch(0.30 0.01 48);border-radius:999px;font:12px system-ui;backdrop-filter:blur(8px)";
const all = await (await fetch("/dev/fixtures")).json();
for (const name of all) {
  const a = document.createElement("a");
  a.href = `?fixture=${name}`;
  a.textContent = name;
  a.style.cssText = `padding:3px 9px;border-radius:999px;text-decoration:none;color:${name === fixtureName ? "oklch(0.18 0.01 45)" : "oklch(0.68 0.01 60)"};background:${name === fixtureName ? "oklch(0.72 0.17 52)" : "transparent"}`;
  bar.append(a);
}
bar.dataset.devOnly = "true";
document.body.append(bar);

// The switcher sits over the bottom of the drawer, where the destructive
// actions are. It is a preview affordance, so it gets out of the way.
const barStyle = document.createElement("style");
barStyle.textContent = "body.is-drawer-open nav[data-dev-only], body.is-setup-mode nav[data-dev-only] { display: none !important; }";
document.head.append(barStyle);
