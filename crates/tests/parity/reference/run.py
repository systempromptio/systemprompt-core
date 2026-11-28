#!/usr/bin/env python3
"""Run golden scenarios through the reference agent and export what it did.

For every scenario and wire protocol: start the scripted instance with the
scenario's script, put a recording relay in front of it, give the reference
agent a fresh home and a fresh copy of the fixture workspace, drive one
session through its headless server, and write its native session export, the
workspace as it left it and the request log to the output directory. The
comparator reads that directory; README.md beside this file is its layout.

Standard library only. The agent's executable and the scripted instance's
executable are arguments; nothing is installed or downloaded here.
"""

import argparse
import json
import os
import queue
import shutil
import signal
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import agent as agent_module  # noqa: E402
import relay as relay_module  # noqa: E402
import scenario as scenario_module  # noqa: E402

FIXTURE_INSTANT = 1700000000
REFUSAL_MARK = "instance: "
IDLE_EVENTS = {
    "session.execution.succeeded": "succeeded",
    "session.execution.failed": "failed",
    "session.execution.interrupted": "interrupted",
}


class RunError(Exception):
    pass


class Instance:
    """The scripted instance's binary: started with a script, stopped by interrupt."""

    def __init__(self, command, script_path, protocol, log_path):
        flag = "--messages-model" if protocol == "messages" else "--chat-model"
        self.log = open(log_path, "w", encoding="utf-8")
        self.process = subprocess.Popen(
            list(command) + ["--port", "0", "--script", script_path, flag, agent_module.MODEL],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=self.log,
            text=True,
        )
        line = self._first_line(120)
        try:
            banner = json.loads(line)
            self.base_url = banner["base_url"]
            self.pat = banner["pat"]
        except (ValueError, KeyError, TypeError):
            self.stop()
            raise RunError("the scripted instance did not print its address line: %r" % line)

    def _first_line(self, limit):
        box = []
        reader = threading.Thread(target=lambda: box.append(self.process.stdout.readline()), daemon=True)
        reader.start()
        reader.join(limit)
        return box[0] if box else ""

    def stop(self):
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGINT)
            try:
                self.process.wait(10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(10)
        self.process.stdout.close()
        self.log.close()


def prepare_workspace(source, target):
    """A fresh copy with one modification time, in a git repository of its own.

    The corpus asks for the single instant (glob and grep order by it). The
    repository is there because the agent snapshots files only inside one; it
    is not part of the compared tree and is left out of the export.
    """
    shutil.copytree(source, target)
    for folder, _, files in os.walk(target):
        for name in files:
            os.utime(os.path.join(folder, name), (FIXTURE_INSTANT, FIXTURE_INSTANT))
    git = ["git", "-c", "user.name=parity", "-c", "user.email=parity@localhost", "-c", "commit.gpgsign=false"]
    env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
    for arguments in (["init", "-q", "-b", "main", "."], ["add", "-A"], ["commit", "-q", "--allow-empty", "-m", "fixture"]):
        subprocess.run(git + arguments, cwd=target, env=env, check=True, capture_output=True)
    for folder, _, files in os.walk(target):
        if ".git" in folder.split(os.sep):
            continue
        for name in files:
            os.utime(os.path.join(folder, name), (FIXTURE_INSTANT, FIXTURE_INSTANT))


def export_tree(source, target, hidden):
    def ignore(folder, names):
        return [name for name in names if folder == source and name in hidden]

    shutil.copytree(source, target, ignore=ignore, symlinks=True)


class Session:
    """One scenario's session, driven from the agent's event stream."""

    def __init__(self, server, scenario, relay, workspace, events_path, turns_total, limit):
        self.server = server
        self.scenario = scenario
        self.relay = relay
        self.workspace = workspace
        self.queue = queue.Queue()
        self.events_file = open(events_path, "w", encoding="utf-8")
        self.turns_total = turns_total
        self.deadline = time.monotonic() + limit
        self.id = None
        self.children = []
        self.user_messages = {}
        self.asks = []
        self.actions = []
        self.idles = []
        self.stops = []
        self.findings = []
        self.responses = 0
        self.texts = set()
        self.reverted = None
        self.pending_asks = list(scenario.asks)
        self.pending_actions = list(scenario.actions)

    def sink(self, event):
        self.queue.put(event)

    def run(self):
        self.server.events(self.sink)
        created = self.server.request(
            "POST",
            "/api/session",
            {
                "location": {"directory": self.workspace},
                "model": {"providerID": agent_module.PROVIDER, "id": agent_module.MODEL},
                "agent": self.scenario.meta.get("agent", "build"),
            },
        )
        self.id = created["data"]["id"]
        self.send(1, None)
        while True:
            event = self.next_event()
            if event is None:
                raise RunError("the agent's event stream ended before the session went idle")
            if self.handle(event):
                break
        self.events_file.close()

    def next_event(self):
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise RunError("the session did not finish inside the time limit")
        try:
            event = self.queue.get(timeout=remaining)
        except queue.Empty:
            raise RunError("the session did not finish inside the time limit")
        if event is not None:
            self.events_file.write(json.dumps(event, sort_keys=True) + "\n")
            self.events_file.flush()
        return event

    def send(self, number, delivery):
        body = {"text": self.scenario.message(number)}
        if delivery:
            body["delivery"] = delivery
        sent = self.server.request("POST", "/api/session/%s/prompt" % self.id, body)
        self.user_messages[number] = sent["data"]["id"]

    def mine(self, session):
        return session == self.id or session in self.children

    def handle(self, event):
        kind = event.get("type")
        data = event.get("data") or {}
        session = data.get("sessionID")
        if kind == "session.created" and data.get("parentID") and self.mine(data.get("parentID")):
            self.children.append(session)
        elif kind == "permission.asked" and self.mine(session):
            self.answer(data)
        elif kind == "session.step.started" and session == self.id:
            self.responses += 1
        elif kind == "session.text.started" and session == self.id and self.responses not in self.texts:
            self.texts.add(self.responses)
            self.act("text", self.responses)
        elif kind in IDLE_EVENTS and session == self.id:
            return self.idle(IDLE_EVENTS[kind])
        return False

    def take_actions(self, on, index):
        taken = [action for action in self.pending_actions if action["on"] == on and action.get("index", 1) == index]
        self.pending_actions = [action for action in self.pending_actions if action not in taken]
        return taken

    def act(self, on, index):
        """Perform the scenario's actions for an observed event; True if any continues the run."""
        continues = False
        for action in self.take_actions(on, index):
            what = action["do"]
            record = dict(action)
            if what == "steer":
                self.send(action["message"], "steer")
                continues = True
            elif what == "prompt":
                self.send(action["message"], None)
                continues = True
            elif what == "abort":
                answer = self.server.request("POST", "/api/session/%s/interrupt" % self.id)
                record["interrupted"] = bool((answer or {}).get("interrupted"))
                continues = True
            elif what == "revert":
                target = self.user_messages.get(action["message"])
                if target is None:
                    raise RunError("the scenario reverts to message %s, which was never sent" % action["message"])
                staged = self.server.request(
                    "POST", "/api/session/%s/revert/stage" % self.id, {"messageID": target, "files": True}
                )
                self.server.request("POST", "/api/session/%s/revert/commit" % self.id)
                self.reverted = {"message": action["message"], "staged": (staged or {}).get("data")}
            else:
                raise RunError("the scenario names an action this runner does not know: %s" % what)
            self.actions.append(record)
        return continues

    def answer(self, request):
        index = len(self.asks) + 1
        self.act("ask", index)
        scripted = self.pending_asks.pop(0) if self.pending_asks else None
        mode = self.scenario.mode
        if scripted is not None:
            reply = {"decision": scripted["reply"]}
            if scripted.get("feedback"):
                reply["message"] = scripted["feedback"]
        elif mode == "allow":
            reply = {"decision": "once"}
        else:
            reply = {"decision": "reject"}
            if mode == "ask":
                self.findings.append("ask %d was raised with no scripted answer left; it was rejected" % index)
        self.asks.append({"index": index, "request": request, "scripted": scripted, "reply": reply})
        self.server.request(
            "POST", "/api/session/%s/permission/%s/reply" % (request["sessionID"], request["id"]), reply
        )

    def idle(self, outcome):
        """The main session stopped running. True when the scenario is over."""
        self.stops.append(outcome)
        if not self.settled():
            return False
        self.idles.append(outcome)
        if self.act("idle", len(self.idles)):
            return False
        return True

    def settled(self):
        """Whether the stop is the session's, not a pause before a child reports back.

        A child session runs in the background and wakes its parent when it
        finishes, so the parent may stop and start again. The stop counts once
        no session is active, the inbox is empty and no event arrives for a
        short while; where scripted turns are still unconsumed the wait is
        longer, because more is certainly expected.
        """
        if not self.children:
            return True
        quiet = 10.0 if self.relay.consumed() < self.turns_total else 1.0
        while True:
            try:
                event = self.queue.get(timeout=quiet)
            except queue.Empty:
                event = None
            if event is None:
                active = (self.server.request("GET", "/api/session/active") or {}).get("data") or []
                active = active if isinstance(active, list) else list(active)
                busy = [item for item in active if self.mine(item.get("id") if isinstance(item, dict) else item)]
                return not busy
            self.events_file.write(json.dumps(event, sort_keys=True) + "\n")
            kind = event.get("type")
            data = event.get("data") or {}
            if kind == "permission.asked" and self.mine(data.get("sessionID")):
                self.answer(data)
            if kind == "session.execution.started" and data.get("sessionID") == self.id:
                return False


def run_one(scenario, protocol, options, table, agent, out_root, work_root):
    out = os.path.join(out_root, scenario.id, protocol)
    work = os.path.join(work_root, scenario.id, protocol)
    for path in (out, work):
        shutil.rmtree(path, ignore_errors=True)
        os.makedirs(path)
    home = os.path.join(work, "home")
    tmp = os.path.join(work, "tmp")
    project = os.path.join(work, "project")
    workspace = os.path.join(project, "workspace")
    for path in (home, tmp, project):
        os.makedirs(path)
    result = {
        "scenario": scenario.id,
        "number": scenario.meta.get("number"),
        "protocol": protocol,
        "status": "error",
        "directory": os.path.relpath(out, out_root),
    }
    record = {"scenario": scenario.id, "protocol": protocol, "model": "%s/%s" % (agent_module.PROVIDER, agent_module.MODEL)}
    instance = relay = server = None
    try:
        prepare_workspace(scenario.workspace, workspace)
        script = scenario_module.translate_turns(scenario, table, "posix")
        script_path = os.path.join(out, "script.json")
        write_json(script_path, script)
        rules = scenario_module.permission_rules(scenario, table)
        instance = Instance(options.instance, script_path, protocol, os.path.join(out, "instance.log"))
        relay = relay_module.Relay(instance.base_url, os.path.join(out, "requests.jsonl"), scenario.title).start()
        env = agent.environment(home, tmp)
        config = agent.config(protocol, relay.base_url, instance.pat, rules)
        os.makedirs(os.path.dirname(agent.config_path(home)))
        write_json(agent.config_path(home), config)
        write_json(os.path.join(out, "agent-config.json"), redact(config))
        server = agent.serve(workspace, env, os.path.join(out, "agent.log"))
        session = Session(
            server, scenario, relay, workspace, os.path.join(out, "events.jsonl"), len(script["turns"]), options.limit
        )
        failure = None
        try:
            session.run()
        except (RunError, agent_module.AgentError) as error:
            failure = str(error)
        record.update(
            {
                "session": session.id,
                "children": session.children,
                "user_messages": session.user_messages,
                "asks": session.asks,
                "actions": session.actions,
                "idles": session.idles,
                "stops": session.stops,
                "revert": session.reverted,
                "findings": session.findings,
            }
        )
        if session.pending_asks:
            session.findings.append("%d scripted answer(s) were never asked for" % len(session.pending_asks))
        if session.pending_actions:
            session.findings.append("%d scripted action(s) never met their event" % len(session.pending_actions))
        if session.id:
            write_json(os.path.join(out, "jobs.json"), server.request("GET", "/api/experimental/job"))
        server.stop()
        server = None
        if session.id:
            with open(os.path.join(out, "session.json"), "w", encoding="utf-8") as handle:
                handle.write(agent.export(session.id, workspace, env))
            if session.children:
                os.makedirs(os.path.join(out, "children"))
            for child in session.children:
                with open(os.path.join(out, "children", child + ".json"), "w", encoding="utf-8") as handle:
                    handle.write(agent.export(child, workspace, env))
        export_tree(workspace, os.path.join(out, "workspace"), {".git", "." + agent.name})
        export_tree(project, os.path.join(out, "outside"), {"workspace"})
        record["relay"] = relay.summary(len(script["turns"]))
        if record["relay"]["unscripted"]:
            session.findings.append("%d model request(s) had no scripted turn" % record["relay"]["unscripted"])
        if record["relay"]["turns_unconsumed"]:
            session.findings.append("%d scripted turn(s) were never requested" % record["relay"]["turns_unconsumed"])
        if failure:
            record["failure"] = failure
            result["reason"] = failure
        else:
            result["status"] = "exported"
        result["findings"] = session.findings
    except (RunError, agent_module.AgentError, scenario_module.ScenarioError, OSError, subprocess.SubprocessError) as error:
        record["failure"] = "%s: %s" % (type(error).__name__, error)
        result["reason"] = record["failure"]
    finally:
        if server:
            server.stop()
        if relay:
            relay.stop()
        if instance:
            instance.stop()
        write_json(os.path.join(out, "run.json"), record)
        if not options.keep_work:
            shutil.rmtree(work, ignore_errors=True)
    return result


def redact(config):
    copy = json.loads(json.dumps(config))
    for provider in copy.get("providers", {}).values():
        if "apiKey" in provider.get("settings", {}):
            provider["settings"]["apiKey"] = "<the scripted instance's test token>"
    return copy


def write_json(path, value):
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")


def corpus_count(root):
    path = os.path.join(root, "corpus.toml")
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as handle:
        return scenario_module.parse_toml(handle.read()).get("scenarios")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--agent", required=True, help="the reference agent's executable")
    parser.add_argument(
        "--instance",
        required=True,
        nargs="+",
        help="the scripted instance's executable (and leading arguments, if it is run through an interpreter)",
    )
    parser.add_argument("--scenarios", required=True, help="the corpus directory")
    parser.add_argument("--out", required=True, help="where the exports are written")
    parser.add_argument("--work", help="scratch directory for homes and workspaces (default: <out>/../work)")
    parser.add_argument("--protocol", action="append", choices=["messages", "chat"], help="default: every protocol a scenario lists")
    parser.add_argument("--only", action="append", default=[], help="a scenario id or number prefix; repeatable")
    parser.add_argument("--limit", type=float, default=180.0, help="seconds one session may take")
    parser.add_argument("--keep-work", action="store_true", help="keep the scratch directories")
    options = parser.parse_args(argv)

    out_root = os.path.abspath(options.out)
    work_root = os.path.abspath(options.work or os.path.join(os.path.dirname(out_root), "work"))
    os.makedirs(out_root, exist_ok=True)
    os.makedirs(work_root, exist_ok=True)
    table = scenario_module.load_table()
    agent = agent_module.Agent(options.agent)
    probe_home = os.path.join(work_root, "version-home")
    os.makedirs(probe_home, exist_ok=True)
    version = agent.version(agent.environment(probe_home, probe_home))

    directories = scenario_module.discover(os.path.abspath(options.scenarios))
    expected = corpus_count(os.path.abspath(options.scenarios))
    problems = []
    if expected is not None and expected != len(directories):
        problems.append("corpus.toml counts %s scenarios; %d were found" % (expected, len(directories)))
    runs = []
    for directory in directories:
        scenario = scenario_module.Scenario(directory)
        if options.only and not any(scenario.id == only or scenario.id.startswith(only) for only in options.only):
            continue
        reason = scenario.skip_reason()
        for protocol in scenario.protocols:
            if options.protocol and protocol not in options.protocol:
                continue
            if reason:
                runs.append(
                    {"scenario": scenario.id, "number": scenario.meta.get("number"), "protocol": protocol, "status": "skipped", "reason": reason}
                )
                print("skipped   %s %s: %s" % (scenario.id, protocol, reason), flush=True)
                continue
            result = run_one(scenario, protocol, options, table, agent, out_root, work_root)
            runs.append(result)
            notes = "; ".join(([result["reason"]] if result.get("reason") else []) + result.get("findings", []))
            print("%-9s %s %s%s" % (result["status"], scenario.id, protocol, ": " + notes if notes else ""), flush=True)
    counts = {}
    for run in runs:
        counts[run["status"]] = counts.get(run["status"], 0) + 1
    write_json(
        os.path.join(out_root, "index.json"),
        {
            "agent": {"version": version, "sha256": agent.sha256()},
            "scenarios_in_corpus": len(directories),
            "counts": counts,
            "problems": problems,
            "runs": runs,
        },
    )
    print("runs: %s" % ", ".join("%d %s" % (count, status) for status, count in sorted(counts.items())), flush=True)
    for problem in problems:
        print("problem: %s" % problem, file=sys.stderr)
    if not runs:
        print("problem: no scenario was selected", file=sys.stderr)
        return 1
    return 1 if problems or counts.get("error") else 0


if __name__ == "__main__":
    sys.exit(main())
