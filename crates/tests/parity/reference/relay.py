"""A recording relay between the reference agent and the scripted instance.

The scripted instance's binary prints its address and serves; it offers no way
to read its request log from outside its process and no way to script the
session-title call. The relay closes both gaps without touching either
product: every model request the agent sends is written to a log, in arrival
order, with the status and the stream it was answered with, and a request that
is a title call is answered here with the scenario's title text so it takes no
scripted turn. Everything else is passed to the scripted instance unchanged
and streamed back as it arrives.

It binds loopback only and forwards to loopback only.
"""

import http.client
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

import sse

HOP_HEADERS = {"connection", "keep-alive", "transfer-encoding", "host", "content-length", "accept-encoding"}
SECRET_HEADERS = {"authorization", "x-api-key"}
REFUSAL = "instance: "


def counts_as_turn(record):
    """A request took a scripted turn unless the instance refused it or it never got there."""
    return record["protocol"] is not None and record["status"] not in (None, 502) and not record.get("unscripted")


def is_title_call(request):
    """A model request that names the session rather than advancing it.

    The corpus's rule (crates/tests/golden/scenarios/README.md, "The title
    call"): it advertises no tools and carries one user message. A compaction
    request advertises none either but carries the conversation.
    """
    if request.get("tools"):
        return False
    users = [message for message in request.get("messages") or [] if message.get("role") == "user"]
    others = [message for message in request.get("messages") or [] if message.get("role") in ("assistant", "tool")]
    return len(users) == 1 and not others


class Relay:
    def __init__(self, upstream, log_path, title_text=None):
        target = urlsplit(upstream)
        if target.hostname not in ("127.0.0.1", "localhost", "::1"):
            raise ValueError("the relay forwards to loopback only, not %s" % target.hostname)
        self.upstream = (target.hostname, target.port)
        self.log_path = log_path
        self.title_text = title_text
        self.lock = threading.Lock()
        self.sequence = 0
        self.records = []
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), self._handler())
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def base_url(self):
        return "http://127.0.0.1:%d" % self.server.server_address[1]

    def start(self):
        self.thread.start()
        return self

    def stop(self):
        self.server.shutdown()
        self.server.server_close()

    def consumed(self):
        """Scripted turns taken so far: requests the instance answered and did not refuse."""
        with self.lock:
            return sum(1 for record in self.records if record["answered_by"] == "scripted-instance" and counts_as_turn(record))

    def summary(self, turns_total):
        with self.lock:
            model = [record for record in self.records if record["protocol"]]
            unscripted = [record for record in model if record.get("unscripted")]
            consumed = sum(1 for record in model if record["answered_by"] == "scripted-instance" and counts_as_turn(record))
            return {
                "requests": len(self.records),
                "model_requests": len(model),
                "title_calls": sum(1 for record in model if record["title_call"]),
                "unscripted": len(unscripted),
                "turns_total": turns_total,
                "turns_consumed": consumed,
                "turns_unconsumed": max(0, turns_total - consumed),
            }

    def _begin(self, record):
        with self.lock:
            record["sequence"] = self.sequence
            self.sequence += 1
            self.records.append(record)
        return record

    def _finish(self, record):
        record["unscripted"] = (
            record["answered_by"] == "scripted-instance" and record["status"] == 400 and REFUSAL in record["response"]
        )
        with self.lock:
            with open(self.log_path, "a", encoding="utf-8") as handle:
                handle.write(json.dumps(record, sort_keys=True) + "\n")

    def _handler(self):
        relay = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, fmt, *args):
                return

            def do_GET(self):
                self.forward(b"")

            def do_POST(self):
                length = int(self.headers.get("content-length") or 0)
                self.forward(self.rfile.read(length) if length else b"")

            def forward(self, raw):
                path = self.path
                protocol = sse.protocol_of(path.split("?")[0])
                try:
                    body = json.loads(raw) if raw else None
                except ValueError:
                    body = None
                record = relay._begin(
                    {
                        "method": self.command,
                        "path": path,
                        "protocol": protocol,
                        "headers": {
                            name.lower(): ("<redacted>" if name.lower() in SECRET_HEADERS else value)
                            for name, value in self.headers.items()
                        },
                        "body": body if body is not None else raw.decode("utf-8", "replace"),
                        "answered_by": "scripted-instance",
                        "title_call": False,
                        "status": None,
                        "response": "",
                        "complete": False,
                    }
                )
                try:
                    if (
                        protocol
                        and relay.title_text is not None
                        and isinstance(body, dict)
                        and is_title_call(body)
                    ):
                        record["answered_by"] = "relay"
                        record["title_call"] = True
                        self.answer_title(record, protocol, body)
                    else:
                        self.pass_through(record, raw)
                finally:
                    relay._finish(record)

            def answer_title(self, record, protocol, body):
                first, last = sse.frames(
                    protocol, body.get("model") or "scripted-model", record["sequence"], [{"text": relay.title_text}], "end"
                )
                text = "".join(first + last)
                record["status"] = 200
                record["response"] = text
                if body.get("stream"):
                    data = text.encode()
                    kind = "text/event-stream"
                else:
                    data = json.dumps(buffered(protocol, body, relay.title_text)).encode()
                    kind = "application/json"
                    record["response"] = data.decode()
                self.send_response(200)
                self.send_header("content-type", kind)
                self.send_header("content-length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                record["complete"] = True

            def pass_through(self, record, raw):
                connection = http.client.HTTPConnection(relay.upstream[0], relay.upstream[1], timeout=3600)
                headers = {name: value for name, value in self.headers.items() if name.lower() not in HOP_HEADERS}
                headers["Content-Length"] = str(len(raw))
                try:
                    connection.request(self.command, self.path, body=raw, headers=headers)
                    response = connection.getresponse()
                except OSError as error:
                    record["status"] = 502
                    record["response"] = "relay: the scripted instance did not answer: %s" % error
                    data = json.dumps({"error": {"message": record["response"]}}).encode()
                    self.send_response(502)
                    self.send_header("content-type", "application/json")
                    self.send_header("content-length", str(len(data)))
                    self.end_headers()
                    self.wfile.write(data)
                    return
                record["status"] = response.status
                self.send_response(response.status)
                for name, value in response.getheaders():
                    if name.lower() not in HOP_HEADERS:
                        self.send_header(name, value)
                self.send_header("connection", "close")
                self.end_headers()
                self.close_connection = True
                chunks = []
                try:
                    while True:
                        chunk = response.read1(65536)
                        if not chunk:
                            record["complete"] = True
                            break
                        chunks.append(chunk)
                        self.wfile.write(chunk)
                        self.wfile.flush()
                except (OSError, http.client.HTTPException):
                    # Why: discard-ok: the agent or the scripted instance closed the stream first; recorded as incomplete.
                    record["complete"] = False
                finally:
                    record["response"] = b"".join(chunks).decode("utf-8", "replace")
                    connection.close()

        return Handler


def buffered(protocol, body, text):
    model = body.get("model") or "scripted-model"
    if protocol == sse.MESSAGES:
        return {
            "id": "msg_relay_title",
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": [{"type": "text", "text": text}],
            "stop_reason": "end_turn",
            "stop_sequence": None,
            "usage": {"input_tokens": 0, "output_tokens": 0},
        }
    return {
        "id": "chatcmpl-relay-title",
        "object": "chat.completion",
        "created": 0,
        "model": model,
        "choices": [{"index": 0, "message": {"role": "assistant", "content": text}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0},
    }
