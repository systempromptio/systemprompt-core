"""Reading a golden scenario and re-expressing it for the reference agent.

The corpus is crates/tests/golden/scenarios; its README is the format
reference. Nothing here knows which product the reference agent is: the tool
and permission names come from tools.json beside this file.
"""

import json
import os
import re

try:
    import tomllib
except ImportError:  # Python older than 3.11
    tomllib = None

HERE = os.path.dirname(os.path.abspath(__file__))
PLACEHOLDER = re.compile(r"\{\{command\.([A-Za-z0-9_]+)\}\}")


class ScenarioError(Exception):
    pass


def parse_toml(text):
    """The TOML subset the corpus is written in (its README, "scenario.toml")."""
    if tomllib is not None:
        return tomllib.loads(text)
    root = {}
    table = root
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("[["):
            table = root
            names = line[2:-2].strip().split(".")
            for name in names[:-1]:
                table = _last(table.setdefault(name, {}))
            table = table.setdefault(names[-1], [])
            table.append({})
            table = table[-1]
        elif line.startswith("["):
            table = root
            for name in line[1:-1].strip().split("."):
                table = _last(table.setdefault(name, {}))
        else:
            key, separator, value = line.partition("=")
            if not separator:
                raise ScenarioError("line %d is not key = value: %s" % (number, raw))
            table[key.strip()] = _value(value.strip(), number)
    return root


def _last(node):
    return node[-1] if isinstance(node, list) else node


def _value(text, number):
    if text.startswith('"'):
        return json.loads(text)
    if text.startswith("'"):
        return text[1:-1]
    if text in ("true", "false"):
        return text == "true"
    if text.startswith("["):
        inner = text[1:-1].strip()
        if not inner:
            return []
        return [_value(part.strip(), number) for part in _split(inner)]
    try:
        return int(text)
    except ValueError:
        raise ScenarioError("line %d has a value outside the subset: %s" % (number, text))


def _split(inner):
    parts, current, quote = [], "", None
    for index, char in enumerate(inner):
        if quote:
            current += char
            if char == quote and (quote == "'" or inner[index - 1] != "\\"):
                quote = None
        elif char in "\"'":
            quote = char
            current += char
        elif char == ",":
            parts.append(current)
            current = ""
        else:
            current += char
    if current.strip():
        parts.append(current)
    return parts


class Scenario:
    def __init__(self, directory):
        self.directory = directory
        with open(os.path.join(directory, "scenario.toml"), encoding="utf-8") as handle:
            self.meta = parse_toml(handle.read())
        self.id = self.meta["id"]
        with open(os.path.join(directory, "prompt.txt"), encoding="utf-8") as handle:
            self.messages = split_messages(handle.read())
        with open(os.path.join(directory, "turns.json"), encoding="utf-8") as handle:
            self.turns = json.load(handle)

    @property
    def products(self):
        return self.meta.get("products", [])

    @property
    def protocols(self):
        return self.meta.get("protocols", [])

    @property
    def asks(self):
        return self.meta.get("ask", [])

    @property
    def actions(self):
        return self.meta.get("action", [])

    @property
    def rules(self):
        return self.meta.get("permission", {}).get("rule", [])

    @property
    def mode(self):
        return self.meta.get("permission", {}).get("mode", "ask")

    @property
    def title(self):
        return self.meta.get("title_call", {}).get("text")

    @property
    def workspace(self):
        return os.path.join(self.directory, "workspace")

    def skip_reason(self):
        if "reference" in self.products:
            return None
        if os.path.exists(os.path.join(self.directory, "verdicts.json")):
            return "harness only: the scenario scripts an instance verdict, which the reference agent does not have"
        return "harness only: scenario.toml does not list the reference product"

    def command(self, name, dialect):
        try:
            return self.meta["command"][name][dialect]
        except KeyError:
            raise ScenarioError("%s names no [command.%s] for the %s dialect" % (self.id, name, dialect))

    def message(self, number):
        try:
            return self.messages[number - 1]
        except IndexError:
            raise ScenarioError("%s has no message %d in prompt.txt" % (self.id, number))


def split_messages(text):
    messages, current = [], []
    for line in text.split("\n"):
        if line == "---":
            messages.append("\n".join(current))
            current = []
        else:
            current.append(line)
    messages.append("\n".join(current))
    return [message.rstrip("\n") for message in messages]


def load_table(path=None):
    with open(path or os.path.join(HERE, "tools.json"), encoding="utf-8") as handle:
        return json.load(handle)


def translate_turns(scenario, table, dialect="posix"):
    """turns.json with the reference agent's tool names and input fields."""

    def substitute(value):
        if isinstance(value, str):
            return PLACEHOLDER.sub(lambda match: scenario.command(match.group(1), dialect), value)
        if isinstance(value, list):
            return [substitute(item) for item in value]
        if isinstance(value, dict):
            return {key: substitute(item) for key, item in value.items()}
        return value

    def tool(name):
        try:
            return table["tools"][name]
        except KeyError:
            raise ScenarioError("%s scripts the logical tool %r, which tools.json does not map" % (scenario.id, name))

    turns = []
    for turn in scenario.turns.get("turns", []):
        turn = json.loads(json.dumps(turn))
        expect = turn.get("expect") or {}
        if "tools_include" in expect:
            expect["tools_include"] = [tool(name)["name"] for name in expect["tools_include"]]
        for item in turn.get("respond") or []:
            call = item.get("tool_call")
            if not call:
                continue
            mapped = tool(call["name"])
            fields = {}
            for key, value in substitute(call["input"]).items():
                if key not in mapped["input"]:
                    raise ScenarioError(
                        "%s: tools.json maps no field %r for the logical tool %r" % (scenario.id, key, call["name"])
                    )
                if mapped["input"][key] is not None:
                    fields[mapped["input"][key]] = value
            renamed = fields != call["input"]
            call["name"] = mapped["name"]
            call["input"] = fields
            if call.get("argument_chunks") and renamed:
                call["argument_chunks"] = rechunk(call["argument_chunks"], fields)
        turns.append(turn)
    return {"turns": turns}


def rechunk(chunks, fields):
    """Split the re-keyed arguments into as many pieces as the script asked for."""
    text = json.dumps(fields, separators=(",", ":"))
    count = max(1, min(len(chunks), len(text)))
    size = -(-len(text) // count)
    return [text[start : start + size] for start in range(0, len(text), size)]


def permission_rules(scenario, table):
    """[[permission.rule]] as (action, resource, effect) in the agent's names."""
    rules = []
    for rule in scenario.rules:
        try:
            action = table["permissions"][rule["permission"]]
        except KeyError:
            raise ScenarioError("%s: tools.json maps no permission %r" % (scenario.id, rule["permission"]))
        rules.append({"action": action, "resource": rule["pattern"], "effect": rule["action"]})
    return rules


def discover(root):
    found = []
    for name in sorted(os.listdir(root)):
        if os.path.isfile(os.path.join(root, name, "scenario.toml")):
            found.append(os.path.join(root, name))
    return found
