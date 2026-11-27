#!/usr/bin/env python3
"""Offline validator for the golden scenario corpus.

Usage: python3 crates/tests/golden/scenarios/validate.py [scenario-dir ...]

Checks every scenario directory beside this file (or the ones named) against
the JSON Schemas in schema/ and against the cross-file rules README.md states.
Standard library only; runs on Python 3.9 and later; touches no network and
writes nothing. Exit status 0 when every scenario validates, 1 otherwise.
"""

import hashlib
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
LOGICAL_TOOLS = ["shell", "read", "write", "edit", "patch", "glob", "grep", "fetch", "todo", "task"]
PLACEHOLDER = re.compile(r"\{\{([a-z_]+(?:\.[a-z_]+)?)\}\}")
ABSOLUTE = re.compile(r"(?:^|[\s\"'=(])(?:/Users/|/home/|/tmp/|/var/|/private/|[A-Za-z]:\\\\?[A-Za-z])")
TIMESTAMP = re.compile(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}")
BANNED_NAME = re.compile(r"(status|plan|report|summary|progress|findings)[^/]*\.(md|txt|org|rst|adoc|pdf|docx)$", re.I)
MAX_FIXTURE_BYTES = 16 * 1024


# ---------------------------------------------------------------- digests

def normalise_text(text):
    """Line endings to LF, trailing blanks off every line, trailing newlines off the end."""
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    lines = [line.rstrip(" \t") for line in text.split("\n")]
    return "\n".join(lines).rstrip("\n")


def text_digest(text):
    return "sha256:" + hashlib.sha256(normalise_text(text).encode("utf-8")).hexdigest()


def file_digest(data):
    """A workspace file: line endings normalised when it is text, raw bytes otherwise."""
    if b"\0" not in data:
        try:
            data = data.decode("utf-8").replace("\r\n", "\n").replace("\r", "\n").encode("utf-8")
        except UnicodeDecodeError:
            pass
    return hashlib.sha256(data).hexdigest()


def read_tree(root):
    """Relative forward-slash path -> bytes for every file under root."""
    tree = {}
    if not os.path.isdir(root):
        return tree
    for base, dirs, files in os.walk(root):
        dirs.sort()
        for name in files:
            full = os.path.join(base, name)
            rel = os.path.relpath(full, root).replace(os.sep, "/")
            with open(full, "rb") as handle:
                tree[rel] = handle.read()
    return tree


def tree_digest(tree):
    lines = ["%s\0%s\n" % (path, file_digest(tree[path])) for path in sorted(tree, key=lambda p: p.encode("utf-8"))]
    return "sha256:" + hashlib.sha256("".join(lines).encode("utf-8")).hexdigest()


def changed_paths(before, after):
    paths = set(before) | set(after)
    changed = [p for p in paths if p not in before or p not in after or file_digest(before[p]) != file_digest(after[p])]
    return sorted(changed, key=lambda p: p.encode("utf-8"))


# ---------------------------------------------------------------- TOML subset

class TomlError(ValueError):
    pass


def _toml_string(text, pos, line_no):
    quote = text[pos]
    out = []
    i = pos + 1
    while i < len(text):
        ch = text[i]
        if ch == quote:
            return "".join(out), i + 1
        if quote == '"' and ch == "\\":
            i += 1
            if i >= len(text):
                break
            escapes = {"n": "\n", "t": "\t", '"': '"', "\\": "\\"}
            if text[i] not in escapes:
                raise TomlError("line %d: unsupported escape \\%s" % (line_no, text[i]))
            out.append(escapes[text[i]])
        else:
            out.append(ch)
        i += 1
    raise TomlError("line %d: unterminated string" % line_no)


def _toml_value(text, pos, line_no):
    while pos < len(text) and text[pos] == " ":
        pos += 1
    if pos >= len(text):
        raise TomlError("line %d: missing value" % line_no)
    ch = text[pos]
    if ch in "\"'":
        return _toml_string(text, pos, line_no)
    if ch == "[":
        items = []
        pos += 1
        while True:
            while pos < len(text) and text[pos] == " ":
                pos += 1
            if pos < len(text) and text[pos] == "]":
                return items, pos + 1
            item, pos = _toml_value(text, pos, line_no)
            items.append(item)
            while pos < len(text) and text[pos] == " ":
                pos += 1
            if pos < len(text) and text[pos] == ",":
                pos += 1
            elif pos >= len(text) or text[pos] != "]":
                raise TomlError("line %d: malformed array" % line_no)
    match = re.match(r"(true|false|-?\d+)(?![\w.])", text[pos:])
    if not match:
        raise TomlError("line %d: unsupported value" % line_no)
    word = match.group(1)
    value = {"true": True, "false": False}.get(word)
    return (int(word) if value is None else value), pos + len(word)


def parse_toml(text):
    """The subset scenario.toml uses: see README.md, section "scenario.toml"."""
    root = {}
    current = root
    for line_no, raw in enumerate(text.split("\n"), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        header = re.match(r"^(\[\[?)([a-z_]+(?:\.[a-z_]+)*)(\]\]?)$", line)
        if header:
            is_array = header.group(1) == "[["
            if (header.group(3) == "]]") != is_array:
                raise TomlError("line %d: mismatched brackets" % line_no)
            keys = header.group(2).split(".")
            node = root
            for key in keys[:-1]:
                node = node.setdefault(key, {})
                if isinstance(node, list):
                    node = node[-1]
            last = keys[-1]
            if is_array:
                node.setdefault(last, [])
                if not isinstance(node[last], list):
                    raise TomlError("line %d: %s is not an array of tables" % (line_no, last))
                node[last].append({})
                current = node[last][-1]
            else:
                if last in node:
                    raise TomlError("line %d: table %s defined twice" % (line_no, header.group(2)))
                node[last] = {}
                current = node[last]
            continue
        pair = re.match(r"^([a-z_]+) = (.*)$", line)
        if not pair:
            raise TomlError("line %d: expected key = value" % line_no)
        key = pair.group(1)
        if key in current:
            raise TomlError("line %d: key %s defined twice" % (line_no, key))
        value, end = _toml_value(pair.group(2), 0, line_no)
        if pair.group(2)[end:].strip():
            raise TomlError("line %d: unexpected text after the value" % line_no)
        current[key] = value
    return root


def load_toml(path, errors):
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    try:
        mine = parse_toml(text)
    except TomlError as error:
        errors.append("%s: %s" % (path, error))
        return None
    try:
        import tomllib
    except ImportError:
        return mine
    try:
        if tomllib.loads(text) != mine:
            errors.append("%s: the subset reader and tomllib disagree; keep to the documented subset" % path)
    except tomllib.TOMLDecodeError as error:
        errors.append("%s: %s" % (path, error))
    return mine


# ---------------------------------------------------------------- JSON Schema subset

TYPES = {
    "string": lambda v: isinstance(v, str),
    "integer": lambda v: isinstance(v, int) and not isinstance(v, bool),
    "boolean": lambda v: isinstance(v, bool),
    "object": lambda v: isinstance(v, dict),
    "array": lambda v: isinstance(v, list),
    "null": lambda v: v is None,
}
KEYWORDS = {
    "$schema", "$id", "title", "description", "definitions", "$ref", "type", "enum", "const", "pattern",
    "minLength", "minimum", "minItems", "items", "properties", "required", "additionalProperties", "oneOf",
}


def check_schema(value, schema, root, where, errors):
    unknown = set(schema) - KEYWORDS
    if unknown:
        errors.append("%s: schema keyword not supported by this validator: %s" % (where, sorted(unknown)))
        return
    if "$ref" in schema:
        target = root
        for part in schema["$ref"].lstrip("#/").split("/"):
            target = target[part]
        check_schema(value, target, root, where, errors)
        return
    if "oneOf" in schema:
        options = schema["oneOf"]
        kind = value.get("kind") if isinstance(value, dict) else None
        tagged = [o for o in options if o.get("properties", {}).get("kind", {}).get("const") == kind]
        if kind is not None and len(tagged) == 1:
            check_schema(value, tagged[0], root, where, errors)
        else:
            passing = 0
            for option in options:
                trial = []
                check_schema(value, option, root, where, trial)
                passing += not trial
            if passing != 1:
                errors.append("%s: matches %d of the %d allowed shapes" % (where, passing, len(options)))
        return
    if "type" in schema:
        names = schema["type"] if isinstance(schema["type"], list) else [schema["type"]]
        if not any(TYPES[name](value) for name in names):
            errors.append("%s: expected %s" % (where, " or ".join(names)))
            return
    if "const" in schema and value != schema["const"]:
        errors.append("%s: must be %r" % (where, schema["const"]))
    if "enum" in schema and value not in schema["enum"]:
        errors.append("%s: %r is not one of %s" % (where, value, schema["enum"]))
    if isinstance(value, str):
        if "pattern" in schema and not re.search(schema["pattern"], value):
            errors.append("%s: %r does not match %s" % (where, value, schema["pattern"]))
        if len(value) < schema.get("minLength", 0):
            errors.append("%s: shorter than %d characters" % (where, schema["minLength"]))
    if TYPES["integer"](value) and "minimum" in schema and value < schema["minimum"]:
        errors.append("%s: below %d" % (where, schema["minimum"]))
    if isinstance(value, list):
        if len(value) < schema.get("minItems", 0):
            errors.append("%s: fewer than %d items" % (where, schema["minItems"]))
        if "items" in schema:
            for index, item in enumerate(value):
                check_schema(item, schema["items"], root, "%s[%d]" % (where, index), errors)
    if isinstance(value, dict):
        properties = schema.get("properties", {})
        for key in schema.get("required", []):
            if key not in value:
                errors.append("%s: missing %s" % (where, key))
        extra = schema.get("additionalProperties", True)
        for key, item in value.items():
            if key in properties:
                check_schema(item, properties[key], root, "%s.%s" % (where, key), errors)
            elif extra is False:
                errors.append("%s: unknown key %s" % (where, key))
            elif isinstance(extra, dict):
                check_schema(item, extra, root, "%s.%s" % (where, key), errors)


def load_schema(name):
    with open(os.path.join(ROOT, "schema", name), encoding="utf-8") as handle:
        return json.load(handle)


def load_json(path, errors):
    try:
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, ValueError) as error:
        errors.append("%s: %s" % (path, error))
        return None


# ---------------------------------------------------------------- scenario rules

def split_prompt(text):
    """prompt.txt: messages separated by a line holding only three hyphens."""
    messages, current = [], []
    for line in text.replace("\r\n", "\n").split("\n"):
        if line == "---":
            messages.append("\n".join(current).strip("\n"))
            current = []
        else:
            current.append(line)
    messages.append("\n".join(current).strip("\n"))
    return messages


def check_hygiene(directory, errors):
    for base, dirs, files in os.walk(directory):
        dirs.sort()
        for name in sorted(files):
            full = os.path.join(base, name)
            rel = os.path.relpath(full, ROOT).replace(os.sep, "/")
            with open(full, "rb") as handle:
                data = handle.read()
            if name == ".gitignore":
                errors.append("%s: use .ignore in a fixture; a .gitignore would hide fixture files from git" % rel)
            if BANNED_NAME.search(rel):
                errors.append("%s: the repository hygiene gate refuses this file name" % rel)
            if len(data) > MAX_FIXTURE_BYTES:
                errors.append("%s: %d bytes, over the %d byte fixture cap" % (rel, len(data), MAX_FIXTURE_BYTES))
            if b"\r" in data:
                errors.append("%s: carriage return found; fixtures are LF only" % rel)
            try:
                text = data.decode("utf-8")
            except UnicodeDecodeError:
                errors.append("%s: not UTF-8" % rel)
                continue
            if data and not data.endswith(b"\n"):
                errors.append("%s: no newline at end of file" % rel)
            if ABSOLUTE.search(text):
                errors.append("%s: absolute path found" % rel)
            if TIMESTAMP.search(text):
                errors.append("%s: timestamp found" % rel)


def substitute(value, commands, where, errors):
    """Names every placeholder in a JSON value; returns the set seen."""
    seen = set()
    if isinstance(value, str):
        for name in PLACEHOLDER.findall(value):
            seen.add(name)
            if name != "protocol" and (not name.startswith("command.") or name.split(".", 1)[1] not in commands):
                errors.append("%s: placeholder {{%s}} has no [command.*] table in scenario.toml" % (where, name))
    elif isinstance(value, list):
        for item in value:
            seen |= substitute(item, commands, where, errors)
    elif isinstance(value, dict):
        for item in value.values():
            seen |= substitute(item, commands, where, errors)
    return seen


def flatten(steps):
    for step in steps:
        yield step
        if step.get("kind") == "subagent":
            for inner in flatten(step.get("steps", [])):
                yield inner


def check_trace(name, scenario, turns, trace, prompts, directory, errors):
    def err(message):
        errors.append("%s/expected/trace.json: %s" % (name, message))

    steps = list(flatten(trace["steps"]))
    requests = [s for s in steps if s["kind"] == "model_request"]
    if len(requests) != len(turns):
        err("%d model_request steps but turns.json has %d turns" % (len(requests), len(turns)))
        return
    scripted_calls = []
    for index, (request, turn) in enumerate(zip(requests, turns), 1):
        wanted = turn.get("expect", {}).get("tool_results", [])
        if request["tool_results"] != wanted:
            err("model_request %d carries %s but turn %d expects %s" % (index, request["tool_results"], index, wanted))
        fault = turn.get("fault")
        status = fault.get("status") if isinstance(fault, dict) else None
        if request.get("status") != status:
            err("model_request %d status %s but turn %d faults with %s" % (index, request.get("status"), index, status))
        texts = [r["text"] for r in turn.get("respond", []) if "text" in r]
        at = next(i for i, step in enumerate(steps) if step is request)
        following = steps[at + 1] if at + 1 < len(steps) else {}
        if following.get("kind") == "subagent":
            following = {}
        expected_kind = "compaction" if request.get("purpose") == "compaction" else "assistant_text"
        if texts:
            if following.get("kind") != expected_kind:
                err("model_request %d is not followed by %s although turn %d has text" % (index, expected_kind, index))
            elif following["digest"] != text_digest("".join(texts)):
                err("%s after model_request %d: digest is not that of turn %d's text" % (expected_kind, index, index))
        elif following.get("kind") in ("assistant_text", "compaction"):
            err("model_request %d is followed by %s but turn %d has no text" % (index, following["kind"], index))
        scripted_calls.extend(r["tool_call"] for r in turn.get("respond", []) if "tool_call" in r)

    traced_calls = [s for s in steps if s["kind"] == "tool_call"]
    if [c["id"] for c in scripted_calls] != [c["call"] for c in traced_calls]:
        err("tool_call steps %s do not follow the script's calls %s" % (
            [c["call"] for c in traced_calls], [c["id"] for c in scripted_calls]))
    else:
        for scripted, traced in zip(scripted_calls, traced_calls):
            if scripted["name"] != traced["tool"] or scripted["input"] != traced["input"]:
                err("tool_call %s differs from the script in tool or input" % scripted["id"])

    results_dir = os.path.join(directory, "expected", "results")
    on_disk = set(os.listdir(results_dir)) if os.path.isdir(results_dir) else set()
    by_call = {}
    for step in steps:
        if step["kind"] in ("tool_call", "permission", "tool_result"):
            by_call.setdefault(step["call"], []).append(step)
    for call, group in by_call.items():
        kinds = [s["kind"] for s in group]
        if kinds != ["tool_call", "permission", "tool_result"]:
            err("call %s has steps %s; expected tool_call, permission, tool_result" % (call, kinds))
            continue
        if len({s["tool"] for s in group}) != 1:
            err("call %s names more than one tool" % call)
        result = group[2]
        file_name = call + ".txt"
        if result["digest"] is None:
            if file_name in on_disk:
                err("call %s has a null digest but expected/results/%s exists" % (call, file_name))
        elif file_name not in on_disk:
            err("call %s has a digest but expected/results/%s is missing" % (call, file_name))
        else:
            with open(os.path.join(results_dir, file_name), encoding="utf-8") as handle:
                if text_digest(handle.read()) != result["digest"]:
                    err("call %s: digest is not that of expected/results/%s" % (call, file_name))
        on_disk.discard(file_name)
        outcome, status = group[1]["outcome"], result["status"]
        allowed = {
            "allowed_by_rule": ("ok", "error", "aborted"), "approved": ("ok", "error", "aborted"),
            "approved_always": ("ok", "error", "aborted"), "denied_by_rule": ("denied",),
            "denied_by_verdict": ("denied",), "rejected": ("rejected",),
        }[outcome]
        if status not in allowed:
            err("call %s: permission %s cannot end in status %s" % (call, outcome, status))
    for stray in sorted(on_disk):
        err("expected/results/%s belongs to no tool_result step" % stray)

    asked = [s for s in steps if s["kind"] == "permission" and s["outcome"] in ("approved", "approved_always", "rejected")]
    asks = scenario.get("ask", [])
    replies = {"approved": "once", "approved_always": "always", "rejected": "reject"}
    if [(a["tool"], a["reply"]) for a in asks] != [(s["tool"], replies[s["outcome"]]) for s in asked]:
        errors.append("%s: the [[ask]] list does not match the trace's approved and rejected permissions" % name)

    for step in steps:
        if step["kind"] in ("user_message", "revert") and not 1 <= step["message"] <= len(prompts):
            err("%s names message %d but prompt.txt has %d" % (step["kind"], step["message"], len(prompts)))
    finishes = [s["reason"] for s in trace["steps"] if s["kind"] == "finish"]
    exit_for = {"end": "ok", "aborted": "aborted", "error": "failed", "stopped": "ok"}
    if not finishes:
        err("no finish step")
    elif exit_for[finishes[-1]] != scenario["end"]["exit"]:
        err("last finish is %s but scenario.toml end.exit is %s" % (finishes[-1], scenario["end"]["exit"]))

    done = [(a["do"], a.get("message")) for a in scenario.get("action", [])]
    traced = []
    for step in trace["steps"]:
        if step["kind"] == "user_message":
            traced.append((step["delivery"], step["message"]))
        elif step["kind"] == "abort":
            traced.append(("abort", None))
        elif step["kind"] == "revert":
            traced.append(("revert", step["message"]))
    if done != traced:
        errors.append("%s: [[action]] entries %s do not match the trace's %s" % (name, done, traced))


def check_scenario(name, schemas, errors):
    directory = os.path.join(ROOT, name)
    before = len(errors)
    for required in ("scenario.toml", "prompt.txt", "turns.json", "expected/trace.json"):
        if not os.path.isfile(os.path.join(directory, required)):
            errors.append("%s: %s is missing" % (name, required))
    for required in ("workspace", "expected/workspace"):
        if not os.path.isdir(os.path.join(directory, required)):
            errors.append("%s: %s/ is missing" % (name, required))
    if len(errors) > before:
        return None
    check_hygiene(directory, errors)

    scenario = load_toml(os.path.join(directory, "scenario.toml"), errors)
    turns_file = load_json(os.path.join(directory, "turns.json"), errors)
    trace = load_json(os.path.join(directory, "expected", "trace.json"), errors)
    verdicts_path = os.path.join(directory, "verdicts.json")
    verdicts = load_json(verdicts_path, errors) if os.path.isfile(verdicts_path) else None
    if scenario is None or turns_file is None or trace is None:
        return None
    check_schema(scenario, schemas["scenario"], schemas["scenario"], name + "/scenario.toml", errors)
    check_schema(turns_file, schemas["turns"], schemas["turns"], name + "/turns.json", errors)
    check_schema(trace, schemas["trace"], schemas["trace"], name + "/expected/trace.json", errors)
    if verdicts is not None:
        check_schema(verdicts, schemas["verdicts"], schemas["verdicts"], name + "/verdicts.json", errors)
    if len(errors) > before:
        return scenario

    if scenario["id"] != name:
        errors.append("%s: scenario.toml id is %s" % (name, scenario["id"]))
    if not name.startswith("%02d-" % scenario["number"]):
        errors.append("%s: directory name does not start with number %02d" % (name, scenario["number"]))
    if verdicts is not None and "reference" in scenario["products"]:
        errors.append("%s: a scripted verdict makes a scenario harness-only" % name)

    with open(os.path.join(directory, "prompt.txt"), encoding="utf-8") as handle:
        prompts = split_prompt(handle.read())
    if any(not message.strip() for message in prompts):
        errors.append("%s/prompt.txt: empty message" % name)
    used = {1} | {a["message"] for a in scenario.get("action", []) if "message" in a}
    if used != set(range(1, len(prompts) + 1)):
        errors.append("%s: prompt.txt has %d messages but the actions use %s" % (name, len(prompts), sorted(used)))
    for action in scenario.get("action", []):
        if (action["do"] in ("steer", "prompt", "revert")) != ("message" in action):
            errors.append("%s: action %s: message is required for steer, prompt and revert only" % (name, action["do"]))

    commands = scenario.get("command", {})
    turns = turns_file["turns"]
    seen = substitute(turns, commands, name + "/turns.json", errors)
    seen |= substitute(trace, commands, name + "/expected/trace.json", errors)
    for unused in sorted(set("command." + c for c in commands) - seen):
        errors.append("%s: [%s] is never used" % (name, unused))

    ids = []
    for index, turn in enumerate(turns, 1):
        where = "%s/turns.json turn %d" % (name, index)
        if "fault" in turn and turn.get("stop") is not None:
            errors.append("%s: a faulted turn has no stop reason" % where)
        if "fault" not in turn and "stop" not in turn:
            errors.append("%s: stop is required unless the turn faults" % where)
        calls = [r["tool_call"] for r in turn.get("respond", []) if "tool_call" in r]
        if "fault" not in turn and (turn.get("stop") == "tool_use") != bool(calls):
            errors.append("%s: stop must be tool_use exactly when the turn calls tools" % where)
        for wanted in turn.get("expect", {}).get("tool_results", []):
            if wanted not in ids:
                errors.append("%s: expects the result of %s, which no earlier turn called" % (where, wanted))
        for call in calls:
            if call["id"] in ids:
                errors.append("%s: call id %s used twice" % (where, call["id"]))
            ids.append(call["id"])
            chunks = call.get("argument_chunks")
            if chunks:
                try:
                    if json.loads("".join(chunks)) != call["input"]:
                        errors.append("%s: argument_chunks of %s do not join to its input" % (where, call["id"]))
                except ValueError:
                    errors.append("%s: argument_chunks of %s do not join to JSON" % (where, call["id"]))

    check_trace(name, scenario, turns, trace, prompts, directory, errors)

    fixture = read_tree(os.path.join(directory, "workspace"))
    after = read_tree(os.path.join(directory, "expected", "workspace"))
    if not fixture:
        errors.append("%s: workspace/ holds no file" % name)
    if trace["workspace"]["tree_digest"] != tree_digest(after):
        errors.append("%s: workspace.tree_digest should be %s" % (name, tree_digest(after)))
    if trace["workspace"]["changed"] != changed_paths(fixture, after):
        errors.append("%s: workspace.changed should be %s" % (name, changed_paths(fixture, after)))
    return scenario


def main(argv):
    schemas = {key: load_schema(key + ".schema.json") for key in ("scenario", "turns", "verdicts", "trace")}
    corpus_errors = []
    corpus = load_toml(os.path.join(ROOT, "corpus.toml"), corpus_errors) or {}
    every = sorted(d for d in os.listdir(ROOT) if os.path.isfile(os.path.join(ROOT, d, "scenario.toml")))
    names = [os.path.basename(os.path.normpath(a)) for a in argv] or every
    failed = 0
    numbers = []
    for name in names:
        errors = []
        scenario = check_scenario(name, schemas, errors)
        if scenario and isinstance(scenario.get("number"), int):
            numbers.append(scenario["number"])
        if errors:
            failed += 1
            print("FAIL %s" % name)
            for error in errors:
                print("     " + error)
        else:
            print("ok   %s" % name)
    if not argv:
        check_hygiene(os.path.join(ROOT, "schema"), corpus_errors)
        if corpus.get("scenarios") != len(every):
            corpus_errors.append("corpus.toml declares %s scenarios; %d found" % (corpus.get("scenarios"), len(every)))
        if sorted(numbers) != list(range(1, len(every) + 1)):
            corpus_errors.append("scenario numbers are not 1..%d without gaps: %s" % (len(every), sorted(numbers)))
    for error in corpus_errors:
        print("FAIL corpus: " + error)
    total = len(names)
    print("%d of %d scenarios valid" % (total - failed, total))
    return 1 if failed or corpus_errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
