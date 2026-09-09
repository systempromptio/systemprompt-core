import test, { beforeEach } from "node:test";
import assert from "node:assert/strict";
import { bridge } from "/assets/js/bridge.js";
import { createListingFetcher } from "/assets/js/services/marketplace-service.js";
import { fixture } from "./fixtures.mjs";

const listing = { plugins: [{ id: "plugin" }], skills: [{ id: "skill", name: "Skill" }] };
const snapshot = (overrides = {}) => ({ ...fixture("healthy"), ...overrides });
const probing = (overrides = {}) => snapshot({
  signed_in: false, identity: { tone: "probing", code: "verifying" }, ...overrides,
});

beforeEach(() => {
  globalThis.document = new EventTarget();
});

function harness(t) {
  const requests = [];
  const counts = [];
  document.addEventListener("mkt:count", (event) => counts.push(event.detail.total));
  t.mock.method(bridge, "marketplaceList", () => new Promise((resolve, reject) => {
    requests.push({ resolve, reject });
  }));
  const fetcher = createListingFetcher();
  return { fetcher, requests, counts };
}

async function load(h, snap = snapshot()) {
  const pending = h.fetcher.maybeFetch(snap);
  h.requests.at(-1).resolve(listing);
  await pending;
}

test("gateway probes and outages preserve skills and counts with an unchanged manifest", async (t) => {
  const h = harness(t);
  await load(h);
  for (const code of ["verifying", "gateway-unreachable"]) {
    await h.fetcher.maybeFetch(probing({ identity: { code } }));
    assert.equal(h.fetcher.state, "ok");
    assert.equal(h.fetcher.listing, listing);
    await h.fetcher.maybeFetch(snapshot());
  }
  assert.equal(h.requests.length, 1);
  assert.deepEqual(h.counts, [2]);
});

test("sign-out clears cached state and the same manifest loads on sign-in", async (t) => {
  const h = harness(t);
  await load(h);
  await h.fetcher.maybeFetch(snapshot({ signed_in: false, verified_identity: null, identity: { code: "signed-out" } }));
  assert.equal(h.fetcher.listing, null);
  assert.equal(h.fetcher.reason, "signed-out");
  await h.fetcher.refresh();
  assert.equal(h.requests.length, 1);
  await load(h);
  assert.equal(h.requests.length, 2);
  assert.equal(h.fetcher.state, "ok");
  assert.deepEqual(h.counts, [2, 0, 2]);
});

test("credential rejection clears the listing even if an old identity is present", async (t) => {
  const h = harness(t);
  await load(h);
  await h.fetcher.maybeFetch(snapshot({ signed_in: false, identity: { code: "token-rejected" } }));
  assert.equal(h.fetcher.listing, null);
  assert.equal(h.fetcher.state, "idle");
});

for (const change of [
  { gateway_url: "https://another.example" },
  { verified_identity: { tenant_id: "another", user_id: "user_1" } },
  { verified_identity: { tenant_id: "tenant_1", user_id: "another" } },
]) {
  test(`a changed session clears old content: ${JSON.stringify(change)}`, async (t) => {
    const h = harness(t);
    await load(h);
    const pending = h.fetcher.maybeFetch(snapshot(change));
    assert.equal(h.fetcher.listing, null);
    assert.equal(h.fetcher.state, "loading");
    h.requests.at(-1).resolve({ skills: [{ id: "new-account" }] });
    await pending;
    assert.equal(h.fetcher.listing.skills[0].id, "new-account");
  });
}

test("token expiry and verification timestamps do not invalidate the account cache", async (t) => {
  const h = harness(t);
  await load(h);
  await h.fetcher.maybeFetch(snapshot({ verified_identity: {
    ...snapshot().verified_identity, exp_unix: 9999999999, verified_at_unix: 9999999990,
  } }));
  assert.equal(h.requests.length, 1);
});

test("late replies cannot restore a signed-out listing or overwrite another account", async (t) => {
  const h = harness(t);
  const old = h.fetcher.maybeFetch(snapshot());
  await h.fetcher.maybeFetch(snapshot({ signed_in: false, verified_identity: null }));
  const fresh = h.fetcher.maybeFetch(snapshot({ gateway_url: "https://another.example" }));
  h.requests[1].resolve({ skills: [{ id: "new" }] });
  await fresh;
  h.requests[0].resolve(listing);
  await old;
  assert.equal(h.fetcher.listing.skills[0].id, "new");
  assert.deepEqual(h.counts, [1]);
});

test("initial requests and background refreshes coalesce repeated snapshots and retries", async (t) => {
  const h = harness(t);
  const first = h.fetcher.maybeFetch(snapshot());
  const duplicate = h.fetcher.maybeFetch(snapshot());
  const retry = h.fetcher.refresh();
  assert.equal(h.requests.length, 1);
  h.requests[0].resolve(listing);
  await Promise.all([first, duplicate, retry]);
  const refresh = h.fetcher.refresh();
  const repeated = h.fetcher.maybeFetch(snapshot());
  assert.equal(h.fetcher.state, "ok");
  assert.equal(h.fetcher.listing, listing);
  assert.equal(h.requests.length, 2);
  h.requests[1].resolve(listing);
  await Promise.all([refresh, repeated]);
});

test("sync completion supersedes an older request even with the same fingerprint", async (t) => {
  const h = harness(t);
  await load(h);
  const old = h.fetcher.refresh();
  const start = h.fetcher.maybeFetch(snapshot({ sync_in_flight: true }));
  const finish = h.fetcher.maybeFetch(snapshot());
  assert.equal(h.requests.length, 3);
  h.requests[2].resolve({ skills: [{ id: "after-sync" }] });
  await finish;
  h.requests[1].reject(new Error("late timeout"));
  await Promise.all([old, start]);
  assert.equal(h.fetcher.listing.skills[0].id, "after-sync");
  assert.equal(h.fetcher.error, null);
});

test("a changed manifest wins over a delayed previous listing", async (t) => {
  const h = harness(t);
  const old = h.fetcher.maybeFetch(snapshot());
  const fresh = h.fetcher.maybeFetch(snapshot({ last_sync_report: { manifest_version: "new" } }));
  h.requests[1].resolve({ skills: [] });
  await fresh;
  h.requests[0].resolve(listing);
  await old;
  assert.deepEqual(h.fetcher.listing.skills, []);
  assert.deepEqual(h.counts, [0]);
});

test("failed background refresh retains the listing and retries without syncing", async (t) => {
  const h = harness(t);
  await load(h);
  const refresh = h.fetcher.refresh();
  h.requests[1].reject(new Error("Could not read files"));
  await refresh;
  assert.equal(h.fetcher.state, "ok");
  assert.equal(h.fetcher.listing, listing);
  assert.equal(h.fetcher.error, "Could not read files");
  assert.deepEqual(h.counts, [2]);
  const retry = h.fetcher.refresh();
  h.requests[2].resolve({ skills: [] });
  await retry;
  assert.equal(h.fetcher.error, null);
  assert.deepEqual(h.fetcher.listing.skills, []);
});

test("a missing initial IPC reply times out and Retry recovers", async (t) => {
  globalThis.window = { ipc: { postMessage: t.mock.fn() } };
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const fetcher = createListingFetcher();
  const pending = fetcher.maybeFetch(snapshot());
  assert.equal(fetcher.state, "loading");
  t.mock.timers.tick(30_000);
  await pending;
  assert.equal(fetcher.state, "error");
  assert.match(fetcher.error, /did not reply within 30s/);
  assert.equal(window.__bridge.pending.size, 0);
  const retry = fetcher.refresh();
  const { id } = JSON.parse(window.ipc.postMessage.mock.calls.at(-1).arguments[0]);
  window.__bridge.reply(id, { ok: true, value: listing });
  await retry;
  assert.equal(fetcher.state, "ok");
  assert.equal(fetcher.listing, listing);
});

test("initial probing has an explicit idle reason and fetches when connected", async (t) => {
  const h = harness(t);
  await h.fetcher.maybeFetch(probing());
  assert.equal(h.fetcher.state, "idle");
  assert.equal(h.fetcher.reason, "verifying");
  assert.equal(h.requests.length, 0);
  await load(h);
  assert.equal(h.fetcher.state, "ok");
});

test("sync completion during a gateway probe refreshes when connectivity returns", async (t) => {
  const h = harness(t);
  await load(h);
  await h.fetcher.maybeFetch(probing({ sync_in_flight: true }));
  await h.fetcher.maybeFetch(probing());
  const connected = h.fetcher.maybeFetch(snapshot());
  assert.equal(h.requests.length, 2);
  h.requests[1].resolve({ skills: [] });
  await connected;
  assert.deepEqual(h.fetcher.listing.skills, []);
});

test("an initial failure retries on the next unchanged connected snapshot", async (t) => {
  const h = harness(t);
  const failed = h.fetcher.maybeFetch(snapshot());
  h.requests[0].reject(new Error("temporary failure"));
  await failed;
  assert.equal(h.fetcher.state, "error");
  await load(h);
  assert.equal(h.fetcher.state, "ok");
  assert.equal(h.requests.length, 2);
});
