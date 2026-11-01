// The inbox pane's row model, read from the same fixtures the preview serves.
// A fixture's `__inbox` is an `InboxPayload` as src/wire/inbox.rs serialises
// it, so these rows are the rows the pane renders in the app.
import test from "node:test";
import assert from "node:assert/strict";

import { EMPTY_INBOX, ageLabel, inboxRows, unreadCount, unreadLabel } from "/assets/js/utils/inbox-rows.js";
import { fixture } from "./fixtures.mjs";

const NOW = Date.parse("2026-10-29T10:00:00Z");
const unread = () => fixture("inbox-unread").__inbox;

test("rows run newest first whatever order the payload carries", () => {
  const payload = unread();
  const rows = inboxRows({ ...payload, items: [...payload.items].reverse() }, NOW);
  const times = rows.map((r) => Date.parse(r.createdAt));
  assert.deepEqual(times, [...times].sort((a, b) => b - a));
});

test("a human mention is unread, openable and not escalatable", () => {
  const [mention] = inboxRows(unread(), NOW);
  assert.equal(mention.attention, "human");
  assert.equal(mention.title, "Priya Natarajan mentioned you");
  assert.equal(mention.isUnread, true);
  assert.equal(mention.canOpen, true);
  assert.equal(mention.canEscalate, false);
  assert.equal(mention.age, "48m");
  assert.equal(mention.count, null, "a single event carries no coalesce badge");
});

test("only agent items offer escalation, and a coalesced item carries its count", () => {
  const agent = inboxRows(unread(), NOW).find((r) => r.attention === "agent");
  assert.ok(agent, "the fixture has an agent item");
  assert.equal(agent.canEscalate, true);
  assert.equal(agent.count, 3);
  assert.equal(agent.viaAgent, "claude-code");
  const others = inboxRows(unread(), NOW).filter((r) => r.attention !== "agent");
  assert.ok(others.every((r) => !r.canEscalate));
});

test("an item without a link cannot be opened, and falls back to its body", () => {
  const escalated = inboxRows(unread(), NOW).find((r) => r.key === "3a7c9e5b-1d46-4f2c-8b18-4e6f8a0b2c34");
  assert.equal(escalated.canOpen, false);
  assert.equal(escalated.attention, "human");
  assert.equal(escalated.isUnread, false);
  assert.match(escalated.text, /MSA/);
});

test("Start session is offered only when the payload says a host can start one", () => {
  assert.ok(inboxRows(unread(), NOW).every((r) => !r.canStartSession));
  const able = inboxRows({ ...unread(), can_start_session: true }, NOW);
  assert.ok(able.every((r) => r.canStartSession));
});

test("the unread count is the server's, never a recount of the bounded list", () => {
  assert.equal(unreadCount(unread()), 2);
  assert.equal(unreadCount({ ...unread(), unread: 57 }), 57);
  assert.equal(unreadCount(fixture("inbox-empty").__inbox), 0);
  assert.equal(unreadCount({ ...unread(), available: false }), 0);
  assert.equal(unreadCount(fixture("inbox-unavailable").__inbox), 0);
  assert.equal(unreadCount(undefined), 0);
  assert.equal(unreadCount({ unread: -1 }), 0);
});

test("an empty or missing payload yields no rows", () => {
  assert.deepEqual(inboxRows(fixture("inbox-empty").__inbox, NOW), []);
  assert.deepEqual(inboxRows(EMPTY_INBOX, NOW), []);
  assert.deepEqual(inboxRows(null, NOW), []);
});

test("an unknown attention string renders as silent rather than an unstyled chip", () => {
  const [item] = unread().items;
  const [row] = inboxRows({ items: [{ ...item, attention: "urgent" }] }, NOW);
  assert.equal(row.attention, "silent");
  assert.equal(row.canEscalate, false);
});

test("ages step through minutes, hours and days", () => {
  assert.equal(ageLabel("2026-10-29T09:59:30Z", NOW), "now");
  assert.equal(ageLabel("2026-10-29T07:40:00Z", NOW), "2h");
  assert.equal(ageLabel("2026-10-27T10:00:00Z", NOW), "2d");
  assert.equal(ageLabel("not a date", NOW), "");
});

test("a capped unread count reads as the cap with a plus", () => {
  const capped = fixture("inbox-capped").__inbox;
  assert.equal(capped.unread_capped, true);
  assert.equal(unreadLabel(capped), "1000+");
  assert.equal(unreadLabel(unread()), "2");
});

test("an empty or unavailable inbox carries no unread label", () => {
  assert.equal(unreadLabel(EMPTY_INBOX), null);
  assert.equal(unreadLabel(fixture("inbox-unavailable").__inbox), null);
});

test("every fixture item carries a delivery state the bridge can emit", () => {
  for (const name of ["inbox-unread", "inbox-capped"]) {
    for (const item of fixture(name).__inbox.items) {
      assert.ok(["pending", "delivered", "held"].includes(item.delivery_state), name);
    }
  }
});
