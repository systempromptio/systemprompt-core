import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { runAction } from "/assets/js/utils/action.js";

const rowKey = (trigger) => trigger.closest("[data-key]")?.dataset.key ?? null;

const say = (id, fallback) => t(id) || fallback;

// Why: the webview can refuse clipboard access (no user-activation, a denied
// permission), and a prompt that silently failed to copy is lost; the row then
// shows the prompt selected in a read-only field to copy by hand.
async function copyPrompt(component, id) {
  const { prompt } = await bridge.invoke("inbox.copy_prompt", { id });
  try {
    await navigator.clipboard.writeText(prompt);
    component.fallback = null;
    return true;
  } catch (err) {
    console.error("clipboard write refused", err);
    component.fallback = { id, prompt };
    return false;
  }
}

function invokeFor(component, name, cmd, argsOf, context) {
  component.registerAction(name, (trigger) => {
    const id = rowKey(trigger);
    return runAction(trigger, { run: () => bridge.invoke(cmd, argsOf(id)), context });
  });
}

export function registerInboxActions(component) {
  invokeFor(component, "inbox-open", "inbox.open", (id) => ({ id }), say("inbox-open", "Open"));
  invokeFor(component, "inbox-read", "inbox.read", (id) => ({ ids: [id] }), say("inbox-mark-read", "Mark read"));
  invokeFor(component, "inbox-read-all", "inbox.read", () => ({ all: true }), say("inbox-mark-all-read", "Mark all read"));
  invokeFor(component, "inbox-load-older", "inbox.load_older", () => ({}), say("inbox-load-older", "Load older"));
  invokeFor(component, "inbox-start", "inbox.start_session", (id) => ({ id }), say("inbox-start-session", "Start session"));
  component.registerAction("inbox-copy", (trigger) => runAction(trigger, {
    run: () => copyPrompt(component, rowKey(trigger)),
    success: (copied) => (copied ? say("inbox-copied", "Prompt copied") : ""),
    context: say("inbox-copy-prompt", "Copy prompt"),
  }));
  component.registerAction("inbox-escalate", (trigger) => { component.escalating = rowKey(trigger); });
  component.registerAction("inbox-escalate-cancel", () => { component.escalating = null; });
  component.registerAction("inbox-escalate-send", (trigger) => {
    const id = rowKey(trigger);
    const note = trigger.closest("[data-key]")?.querySelector("[data-note]")?.value ?? "";
    return runAction(trigger, {
      run: async () => {
        await bridge.invoke("inbox.escalate", { id, note: note.trim() || null });
        component.escalating = null;
      },
      success: say("inbox-escalated", "Escalated"),
      context: say("inbox-escalate", "Escalate"),
    });
  });
}
