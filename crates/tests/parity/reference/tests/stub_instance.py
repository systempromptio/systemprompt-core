#!/usr/bin/env python3
"""A STUB of the scripted instance's binary. Not the scripted instance.

It stands in for `systemprompt-scripted-instance` where that binary is not
built, so the runner can be exercised without cargo. It takes the same flags,
prints the same JSON line and serves the two model routes only, with the
matching, guard and fault rules of
documentation/future/harness/testing/02-scripted-instance.md section 3. It
signs nothing and serves no instance route. A parity run uses the real binary.
"""

import argparse
import json
import os
import secrets
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import sse  # noqa: E402


class Script:
    def __init__(self, turns):
        self.turns = turns
        self.consumed = 0
        self.lock = threading.Lock()

    def take(self, request):
        with self.lock:
            if self.consumed >= len(self.turns):
                return None, "the script has no turn left for this request (%d consumed)" % self.consumed
            turn = self.turns[self.consumed]
            mismatch = guard(turn.get("expect") or {}, request)
            if mismatch:
                return None, "turn %d: %s" % (self.consumed, mismatch)
            self.consumed += 1
            return (self.consumed - 1, turn), None


def advertised_tools(request):
    names = []
    for tool in request.get("tools") or []:
        name = tool.get("name") or (tool.get("function") or {}).get("name")
        if name:
            names.append(name)
    return names


def tool_results(request):
    ids = []
    for message in request.get("messages") or []:
        if message.get("role") == "tool" and message.get("tool_call_id"):
            ids.append(message["tool_call_id"])
        content = message.get("content")
        for block in content if isinstance(content, list) else []:
            if isinstance(block, dict) and block.get("type") == "tool_result" and block.get("tool_use_id"):
                ids.append(block["tool_use_id"])
    return ids


def guard(expect, request):
    tools = advertised_tools(request)
    results = tool_results(request)
    wanted_tools = expect.get("tools_include") or []
    wanted_results = expect.get("tool_results") or []
    if all(name in tools for name in wanted_tools) and all(call in results for call in wanted_results):
        return None
    return "the turn expects tools %s and tool results %s; the request advertises tools %s and carries tool results %s" % (
        wanted_tools,
        wanted_results,
        tools,
        results,
    )


def make_handler(script, pat):
    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, fmt, *args):
            sys.stderr.write("stub instance: " + fmt % args + "\n")

        def do_GET(self):
            self.send_json(404, {"error": "the stub serves the model routes only"})

        def do_POST(self):
            length = int(self.headers.get("content-length") or 0)
            raw = self.rfile.read(length) if length else b""
            protocol = sse.protocol_of(self.path.split("?")[0])
            if protocol is None:
                self.send_json(404, {"error": "the stub serves the model routes only"})
                return
            bearer = self.headers.get("authorization") or ""
            key = self.headers.get("x-api-key") or ""
            if bearer != "Bearer " + pat and key != pat:
                self.send_json(401, sse.error_body(protocol, 401, "stub instance: the personal access token is required"))
                return
            try:
                request = json.loads(raw or b"{}")
            except ValueError:
                request = {}
            taken, refusal = script.take(request)
            if taken is None:
                self.send_json(400, sse.error_body(protocol, 400, "stub instance: " + refusal))
                return
            index, turn = taken
            fault = turn.get("fault")
            if isinstance(fault, dict) and "status" in fault:
                body = sse.error_body(protocol, fault["status"], "stub instance: the turn answers with this status")
                self.send_json(fault["status"], body, {"retry-after-ms": "1"})
                return
            body, terminal = sse.frames(
                protocol, request.get("model") or "scripted-model", index, turn.get("respond") or [], turn.get("stop"), turn.get("usage")
            )
            self.send_response(200)
            self.send_header("content-type", "text/event-stream")
            self.send_header("cache-control", "no-cache")
            self.send_header("connection", "close")
            self.end_headers()
            self.close_connection = True
            try:
                for frame in body:
                    self.wfile.write(frame.encode())
                    self.wfile.flush()
                if isinstance(fault, dict) and "stall_ms" in fault:
                    deadline = time.monotonic() + fault["stall_ms"] / 1000.0
                    while time.monotonic() < deadline:
                        time.sleep(0.05)
                        self.wfile.write(b"")
                        self.wfile.flush()
                        if self.connection_lost():
                            return
                for frame in terminal:
                    self.wfile.write(frame.encode())
                self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                return

        def connection_lost(self):
            import select

            readable, _, _ = select.select([self.connection], [], [], 0)
            if not readable:
                return False
            try:
                return self.connection.recv(1, 2) == b""  # MSG_PEEK
            except OSError:
                return True

        def send_json(self, status, payload, headers=None):
            data = json.dumps(payload).encode()
            self.send_response(status)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(data)))
            for name, value in (headers or {}).items():
                self.send_header(name, value)
            self.end_headers()
            self.wfile.write(data)

    return Handler


def main():
    parser = argparse.ArgumentParser(description="A stub of the scripted instance's binary (model routes only)")
    parser.add_argument("--port", type=int, default=0)
    parser.add_argument("--script")
    parser.add_argument("--verdicts")
    parser.add_argument("--messages-model", action="append", default=[])
    parser.add_argument("--chat-model", action="append", default=[])
    args = parser.parse_args()
    turns = []
    if args.script:
        with open(args.script, encoding="utf-8") as handle:
            turns = json.load(handle).get("turns") or []
    pat = "stub-pat-" + secrets.token_hex(16)
    server = ThreadingHTTPServer(("127.0.0.1", args.port), make_handler(Script(turns), pat))
    server.daemon_threads = True
    print(json.dumps({"base_url": "http://127.0.0.1:%d" % server.server_address[1], "pat": pat}), flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
