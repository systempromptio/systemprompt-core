"""Tests of the reference runner's own parts. No agent and no cargo: the model
side is the stub beside this file, which is named as a stub and is not the
scripted instance.

    python3 -m unittest discover -s crates/tests/parity/reference/tests
"""

import http.client
import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from urllib.parse import urlsplit

HERE = os.path.dirname(os.path.abspath(__file__))
RUNNER = os.path.dirname(HERE)
CORPUS = os.path.join(os.path.dirname(os.path.dirname(RUNNER)), "golden", "scenarios")
sys.path.insert(0, RUNNER)

import agent  # noqa: E402
import network_guard  # noqa: E402
import relay  # noqa: E402
import run  # noqa: E402
import scenario  # noqa: E402
import sse  # noqa: E402

HARNESS_ONLY = ["16-verdict-deny", "17-verdict-timeout"]


def post(base_url, path, body, key):
    target = urlsplit(base_url)
    connection = http.client.HTTPConnection(target.hostname, target.port, timeout=30)
    connection.request(
        "POST", path, body=json.dumps(body).encode(), headers={"content-type": "application/json", "x-api-key": key}
    )
    response = connection.getresponse()
    text = response.read().decode()
    connection.close()
    return response.status, text


class Corpus(unittest.TestCase):
    def setUp(self):
        self.table = scenario.load_table()
        self.scenarios = [scenario.Scenario(path) for path in scenario.discover(CORPUS)]

    def test_the_corpus_count_is_the_pinned_one(self):
        self.assertEqual(run.corpus_count(CORPUS), len(self.scenarios))
        self.assertGreater(len(self.scenarios), 0)

    def test_only_the_verdict_scenarios_are_skipped(self):
        skipped = [item.id for item in self.scenarios if item.skip_reason()]
        self.assertEqual(skipped, HARNESS_ONLY)
        for item in self.scenarios:
            if item.id in HARNESS_ONLY:
                self.assertIn("verdict", item.skip_reason())

    def test_every_scenario_translates_with_the_table(self):
        advertised = {entry["name"] for entry in self.table["tools"].values()}
        for item in self.scenarios:
            script = scenario.translate_turns(item, self.table)
            self.assertEqual(len(script["turns"]), len(item.turns["turns"]))
            text = json.dumps(script)
            self.assertNotIn("{{command.", text, item.id)
            for turn in script["turns"]:
                for name in (turn.get("expect") or {}).get("tools_include", []):
                    self.assertIn(name, advertised)
                for entry in turn.get("respond") or []:
                    call = entry.get("tool_call")
                    if call:
                        self.assertIn(call["name"], advertised)
                        if call.get("argument_chunks"):
                            self.assertEqual(json.loads("".join(call["argument_chunks"])), call["input"])

    def test_edit_fields_take_the_reference_names(self):
        item = next(entry for entry in self.scenarios if entry.id == "03-edit-file")
        call = scenario.translate_turns(item, self.table)["turns"][0]["respond"][-1]["tool_call"]
        self.assertEqual(sorted(call["input"]), ["newString", "oldString", "path"])

    def test_a_field_the_reference_tool_lacks_is_dropped(self):
        item = next(entry for entry in self.scenarios if entry.id == "09-shell-success")
        call = scenario.translate_turns(item, self.table)["turns"][0]["respond"][-1]["tool_call"]
        self.assertEqual(sorted(call["input"]), ["command"])

    def test_rules_take_the_reference_permission_names(self):
        item = next(entry for entry in self.scenarios if entry.id == "13-denied-by-rule")
        self.assertEqual(
            scenario.permission_rules(item, self.table), [{"action": "edit", "resource": "secrets/*", "effect": "deny"}]
        )

    def test_messages_split_on_the_separator_line(self):
        self.assertEqual(scenario.split_messages("one\n---\ntwo\nlines\n"), ["one", "two\nlines"])


class TomlSubset(unittest.TestCase):
    def test_the_subset_reader_reads_what_the_corpus_uses(self):
        text = (
            'id = "x"\nnumber = 3\nlist = ["a", "b"]\n\n[permission]\nmode = "ask"\n\n'
            '[[permission.rule]]\npermission = "edit"\n\n[[ask]]\nreply = "once"\n\n[[ask]]\nreply = "reject"\n\n'
            "[command.c]\nposix = 'echo \"hi\" $x'\n\n[end]\nflag = true\n"
        )
        saved = scenario.tomllib
        try:
            scenario.tomllib = None
            parsed = scenario.parse_toml(text)
        finally:
            scenario.tomllib = saved
        self.assertEqual(parsed["number"], 3)
        self.assertEqual(parsed["list"], ["a", "b"])
        self.assertEqual(parsed["permission"], {"mode": "ask", "rule": [{"permission": "edit"}]})
        self.assertEqual([entry["reply"] for entry in parsed["ask"]], ["once", "reject"])
        self.assertEqual(parsed["command"]["c"]["posix"], 'echo "hi" $x')
        self.assertIs(parsed["end"]["flag"], True)
        if saved is not None:
            self.assertEqual(parsed, saved.loads(text))


class TitleCall(unittest.TestCase):
    def test_a_toolless_single_message_request_is_a_title_call(self):
        self.assertTrue(relay.is_title_call({"messages": [{"role": "system", "content": "s"}, {"role": "user", "content": "u"}]}))

    def test_a_request_with_tools_or_history_is_not(self):
        self.assertFalse(relay.is_title_call({"tools": [{"name": "read"}], "messages": [{"role": "user", "content": "u"}]}))
        self.assertFalse(
            relay.is_title_call(
                {"messages": [{"role": "user", "content": "u"}, {"role": "assistant", "content": "a"}, {"role": "user", "content": "v"}]}
            )
        )


class RelayAgainstTheStub(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        script = os.path.join(self.folder.name, "script.json")
        with open(script, "w", encoding="utf-8") as handle:
            json.dump(
                {
                    "turns": [
                        {
                            "expect": {"tools_include": ["read"]},
                            "respond": [{"tool_call": {"id": "t1", "name": "read", "input": {"path": "a"}}}],
                            "stop": "tool_use",
                        },
                        {"fault": {"status": 429}},
                        {"expect": {"tool_results": ["t1"]}, "respond": [{"text": "Done."}], "stop": "end"},
                    ]
                },
                handle,
            )
        self.instance = run.Instance(
            [sys.executable, os.path.join(HERE, "stub_instance.py")], script, "messages", os.path.join(self.folder.name, "stub.log")
        )
        self.log = os.path.join(self.folder.name, "requests.jsonl")
        self.relay = relay.Relay(self.instance.base_url, self.log, "A title").start()

    def tearDown(self):
        self.relay.stop()
        self.instance.stop()
        self.folder.cleanup()

    def test_turns_title_faults_and_refusals_are_told_apart(self):
        tools = [{"name": "read", "input_schema": {}}]
        user = {"role": "user", "content": "go"}
        key = self.instance.pat
        status, text = post(self.relay.base_url, "/v1/messages", {"stream": True, "messages": [user]}, key)
        self.assertEqual(status, 200)
        self.assertIn("A title", text)
        status, text = post(self.relay.base_url, "/v1/messages", {"stream": True, "tools": tools, "messages": [user]}, key)
        self.assertEqual(status, 200)
        self.assertIn('"tool_use"', text)
        self.assertIn("message_stop", text)
        status, _ = post(self.relay.base_url, "/v1/messages", {"stream": True, "tools": tools, "messages": [user]}, key)
        self.assertEqual(status, 429)
        status, text = post(self.relay.base_url, "/v1/messages", {"stream": True, "tools": tools, "messages": [user]}, key)
        self.assertEqual(status, 400)
        self.assertIn("tool results", text)
        result = {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": "x"}]}
        status, text = post(
            self.relay.base_url, "/v1/chat/completions", {"stream": True, "tools": tools, "messages": [user, result]}, key
        )
        self.assertEqual(status, 200)
        self.assertIn("[DONE]", text)
        summary = self.relay.summary(3)
        self.assertEqual(
            summary,
            {
                "requests": 5,
                "model_requests": 5,
                "title_calls": 1,
                "unscripted": 1,
                "turns_total": 3,
                "turns_consumed": 3,
                "turns_unconsumed": 0,
            },
        )
        with open(self.log, encoding="utf-8") as handle:
            records = [json.loads(line) for line in handle]
        self.assertEqual([record["sequence"] for record in records], [0, 1, 2, 3, 4])
        self.assertEqual([record["answered_by"] for record in records], ["relay"] + ["scripted-instance"] * 4)
        self.assertEqual(records[0]["headers"]["x-api-key"], "<redacted>")
        self.assertNotIn(key, json.dumps(records))

    def test_a_request_without_the_token_takes_no_turn(self):
        status, _ = post(self.relay.base_url, "/v1/messages", {"tools": [{"name": "read"}], "messages": []}, "wrong")
        self.assertEqual(status, 401)

    def test_the_relay_refuses_an_upstream_that_is_not_loopback(self):
        with self.assertRaises(ValueError):
            relay.Relay("http://192.0.2.1:80", self.log)


class Encoders(unittest.TestCase):
    def test_both_protocols_carry_text_tool_call_and_stop(self):
        respond = [{"text": "hi"}, {"tool_call": {"id": "t1", "name": "read", "input": {"path": "a"}, "argument_chunks": ['{"pa', 'th":"a"}']}}]
        body, terminal = sse.frames(sse.MESSAGES, "m", 0, respond)
        joined = "".join(body + terminal)
        self.assertIn('"partial_json":"{\\"pa"', joined)
        self.assertIn('"stop_reason":"tool_use"', joined)
        body, terminal = sse.frames(sse.CHAT, "m", 0, respond)
        joined = "".join(body + terminal)
        self.assertIn('"finish_reason":"tool_calls"', joined)
        self.assertTrue(joined.endswith("data: [DONE]\n\n"))


class AgentProfile(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.binary = os.path.join(self.folder.name, "refagent")
        with open(self.binary, "w", encoding="utf-8") as handle:
            handle.write("#!/bin/sh\necho refagent v0\n")
        os.chmod(self.binary, os.stat(self.binary).st_mode | stat.S_IXUSR)
        self.agent = agent.Agent(self.binary)

    def tearDown(self):
        self.folder.cleanup()

    def test_names_come_from_the_executable(self):
        self.assertEqual(self.agent.config_path("/h"), "/h/.config/refagent/refagent.json")
        config = self.agent.config(sse.CHAT, "http://127.0.0.1:9", "k", [])
        provider = config["providers"][agent.PROVIDER]
        self.assertEqual(provider["package"], "@refagent/ai/providers/openai-compatible")
        self.assertEqual(provider["settings"]["baseURL"], "http://127.0.0.1:9/v1")
        self.assertEqual(config["update"], "disable")
        messages = self.agent.config(sse.MESSAGES, "http://127.0.0.1:9", "k", [])
        self.assertEqual(messages["providers"][agent.PROVIDER]["package"], "@refagent/ai/providers/anthropic-compatible")

    def test_a_provider_outside_loopback_is_refused(self):
        with self.assertRaises(agent.AgentError):
            self.agent.config(sse.CHAT, "https://api.example.com", "k", [])

    def test_the_environment_is_built_from_nothing(self):
        os.environ["PARITY_TEST_API_KEY"] = "must-not-leak"
        try:
            env = self.agent.environment("/h", "/t")
        finally:
            del os.environ["PARITY_TEST_API_KEY"]
        self.assertNotIn("PARITY_TEST_API_KEY", env)
        self.assertEqual([name for name in env if name.endswith("API_KEY") or name.endswith("TOKEN")], [])
        self.assertEqual(env["REFAGENT_DISABLE_MODELS_FETCH"], "1")
        self.assertEqual(env["HOME"], "/h")

    def test_the_version_is_what_the_executable_prints(self):
        self.assertEqual(self.agent.version(self.agent.environment(self.folder.name, self.folder.name)), "refagent v0")

    def test_the_stored_configuration_copy_holds_no_token(self):
        config = self.agent.config(sse.CHAT, "http://127.0.0.1:9", "secret-token", [])
        self.assertNotIn("secret-token", json.dumps(run.redact(config)))


class NetworkGuard(unittest.TestCase):
    LOOPBACK = '10 connect(5, {sa_family=AF_INET, sin_port=htons(40123), sin_addr=inet_addr("127.0.0.1")}, 16) = 0'

    def test_loopback_only_passes(self):
        six = '10 connect(6, {sa_family=AF_INET6, sin6_port=htons(4096), sin6_flowinfo=htonl(0), inet_pton(AF_INET6, "::1", &sin6_addr), sin6_scope_id=0}, 28) = 0'
        unix = '10 connect(7, {sa_family=AF_UNIX, sun_path="/run/x"}, 110) = 0'
        self.assertEqual(network_guard.inspect([self.LOOPBACK, six, unix]), ([], 2))

    def test_an_outside_address_is_a_violation(self):
        line = '11 connect(5, {sa_family=AF_INET, sin_port=htons(443), sin_addr=inet_addr("203.0.113.9")}, 16) = -1 ENETUNREACH'
        violations, _ = network_guard.inspect([self.LOOPBACK, line])
        self.assertEqual(len(violations), 1)
        self.assertIn("203.0.113.9", violations[0])

    def test_an_outside_ipv6_address_is_a_violation(self):
        line = '11 connect(5, {sa_family=AF_INET6, sin6_port=htons(443), sin6_flowinfo=htonl(0), inet_pton(AF_INET6, "2001:db8::1", &sin6_addr), sin6_scope_id=0}, 28) = -1'
        self.assertEqual(len(network_guard.inspect([line])[0]), 1)

    def test_a_name_lookup_is_a_violation_even_on_loopback(self):
        line = '11 connect(5, {sa_family=AF_INET, sin_port=htons(53), sin_addr=inet_addr("127.0.0.53")}, 16) = 0'
        self.assertIn("name lookup", network_guard.inspect([line])[0][0])

    def test_an_empty_trace_fails(self):
        with tempfile.NamedTemporaryFile("w", suffix=".trace", delete=False) as handle:
            path = handle.name
        try:
            done = subprocess.run([sys.executable, os.path.join(RUNNER, "network_guard.py"), path], capture_output=True, text=True)
        finally:
            os.unlink(path)
        self.assertEqual(done.returncode, 1)
        self.assertIn("nothing was traced", done.stderr)


if __name__ == "__main__":
    unittest.main()
