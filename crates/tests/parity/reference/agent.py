"""The reference agent as a process: its isolated environment, its
configuration file, its headless server and that server's HTTP interface.

Nothing here names the product. Every product-specific name is derived from
the file name of the executable the caller passes: the configuration
directory and file, the prefix of its environment switches and the scope of
its provider packages. The workflow that installs the executable is the one
place that names it. README.md beside this file records each key and command
used here and where the product documents it.
"""

import base64
import hashlib
import http.client
import json
import os
import signal
import subprocess
import threading
import time
from urllib.parse import urlsplit

import sse

PROVIDER = "scripted"
MODEL = "scripted-model"
CONTEXT_WINDOW = 200000
OUTPUT_LIMIT = 8192
PACKAGES = {sse.MESSAGES: "anthropic-compatible", sse.CHAT: "openai-compatible"}


class AgentError(Exception):
    pass


class Agent:
    def __init__(self, binary):
        self.binary = os.path.abspath(binary)
        if not os.access(self.binary, os.X_OK):
            raise AgentError("%s is not an executable file" % self.binary)
        name = os.path.basename(self.binary)
        self.name = name[:-4] if name.lower().endswith(".exe") else name
        self.switch = self.name.upper().replace("-", "_") + "_"

    def sha256(self):
        digest = hashlib.sha256()
        with open(self.binary, "rb") as handle:
            for block in iter(lambda: handle.read(1 << 20), b""):
                digest.update(block)
        return digest.hexdigest()

    def environment(self, home, tmp):
        """Built from nothing, so no provider variable of the caller's leaks in."""
        return {
            "HOME": home,
            "XDG_CONFIG_HOME": os.path.join(home, ".config"),
            "XDG_DATA_HOME": os.path.join(home, ".local", "share"),
            "XDG_CACHE_HOME": os.path.join(home, ".cache"),
            "XDG_STATE_HOME": os.path.join(home, ".local", "state"),
            "TMPDIR": tmp,
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "LANG": "C.UTF-8",
            "TERM": "dumb",
            "NO_COLOR": "1",
            "NO_PROXY": "127.0.0.1,localhost,::1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            self.switch + "DISABLE_AUTOUPDATE": "1",
            self.switch + "DISABLE_MODELS_FETCH": "1",
            self.switch + "DISABLE_CLAUDE_CODE": "1",
        }

    def config_path(self, home):
        return os.path.join(home, ".config", self.name, self.name + ".json")

    def config(self, protocol, base_url, key, rules):
        if urlsplit(base_url).hostname != "127.0.0.1":
            raise AgentError("the provider base URL must be loopback, not %s" % base_url)
        return {
            "update": "disable",
            "model": "%s/%s" % (PROVIDER, MODEL),
            "snapshots": True,
            "formatter": False,
            "experimental": {"todo": True},
            "permissions": rules,
            "providers": {
                PROVIDER: {
                    "name": "Scripted instance",
                    "package": "@%s/ai/providers/%s" % (self.name, PACKAGES[protocol]),
                    "settings": {"baseURL": base_url + "/v1", "apiKey": key},
                    "models": {
                        MODEL: {"name": "Scripted model", "limit": {"context": CONTEXT_WINDOW, "output": OUTPUT_LIMIT}}
                    },
                }
            },
        }

    def version(self, env):
        done = subprocess.run([self.binary, "--version"], env=env, capture_output=True, text=True, timeout=60)
        return done.stdout.strip()

    def serve(self, cwd, env, log_path):
        return Server(self, cwd, env, log_path)

    def export(self, session, cwd, env):
        """The documented native export: `session export <id>` with a private server."""
        done = subprocess.run(
            [self.binary, "session", "export", session, "--standalone"],
            cwd=cwd,
            env=env,
            capture_output=True,
            text=True,
            timeout=120,
            stdin=subprocess.DEVNULL,
        )
        if done.returncode != 0:
            raise AgentError("session export exited %d: %s" % (done.returncode, done.stderr.strip()[-400:]))
        return done.stdout


class Server:
    """`serve` on an ephemeral loopback port, and requests against it."""

    def __init__(self, agent, cwd, env, log_path):
        self.agent = agent
        self.cwd = cwd
        self.env = env
        self.out_path = log_path + ".stdout"
        self.out = open(self.out_path, "w", encoding="utf-8")
        self.log = open(log_path, "w", encoding="utf-8")
        self.process = subprocess.Popen(
            [agent.binary, "serve", "--hostname", "127.0.0.1", "--port", "0", "--print-logs"],
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=self.out,
            stderr=self.log,
            start_new_session=True,
        )
        self.url = None
        self.password = None
        self._await_banner()
        target = urlsplit(self.url)
        if target.hostname != "127.0.0.1":
            self.stop()
            raise AgentError("the agent's server is not on loopback: %s" % self.url)
        self.address = (target.hostname, target.port)
        token = base64.b64encode(("%s:%s" % (agent.name, self.password)).encode()).decode()
        self.headers = {"authorization": "Basic " + token}

    def _await_banner(self, limit=120.0):
        deadline = time.monotonic() + limit
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise AgentError("the agent's server exited %d before it listened" % self.process.returncode)
            with open(self.out_path, encoding="utf-8") as handle:
                for line in handle:
                    words = line.split()
                    if "listening" in words:
                        self.url = words[-1].rstrip("/")
                    if "password" in words:
                        self.password = words[-1]
            if self.url and self.password:
                return
            time.sleep(0.05)
        self.stop()
        raise AgentError("the agent's server did not print its address and password")

    def request(self, method, path, body=None, timeout=120):
        connection = http.client.HTTPConnection(self.address[0], self.address[1], timeout=timeout)
        headers = dict(self.headers)
        data = None
        if body is not None:
            data = json.dumps(body).encode()
            headers["content-type"] = "application/json"
        try:
            connection.request(method, path, body=data, headers=headers)
            response = connection.getresponse()
            raw = response.read().decode("utf-8", "replace")
        finally:
            connection.close()
        try:
            parsed = json.loads(raw) if raw else None
        except ValueError:
            parsed = raw
        if response.status >= 400:
            raise AgentError("%s %s answered %d: %s" % (method, path, response.status, raw[:400]))
        return parsed

    def events(self, sink):
        """Start reading the event stream; each event goes to sink(dict)."""
        ready = threading.Event()

        def read():
            connection = http.client.HTTPConnection(self.address[0], self.address[1], timeout=None)
            try:
                connection.request("GET", "/api/event", headers=dict(self.headers, accept="text/event-stream"))
                response = connection.getresponse()
                ready.set()
                for raw in response:
                    line = raw.decode("utf-8", "replace").rstrip("\r\n")
                    if line.startswith("data:"):
                        try:
                            sink(json.loads(line[5:].strip()))
                        except ValueError:
                            continue
            except (OSError, http.client.HTTPException):
                pass
            finally:
                ready.set()
                sink(None)
                connection.close()

        thread = threading.Thread(target=read, daemon=True)
        thread.start()
        ready.wait(30)
        return thread

    def stop(self):
        if self.process.poll() is None:
            self._signal(signal.SIGTERM)
            try:
                self.process.wait(20)
            except subprocess.TimeoutExpired:
                pass
        self._signal(signal.SIGKILL)
        try:
            self.process.wait(10)
        except subprocess.TimeoutExpired:
            pass
        self.out.close()
        self.log.close()

    def _signal(self, number):
        try:
            os.killpg(self.process.pid, number)
        except (ProcessLookupError, PermissionError):
            pass
