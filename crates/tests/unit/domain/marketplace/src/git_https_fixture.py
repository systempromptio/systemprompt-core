import http.server
import os
import pathlib
import ssl
import subprocess
import sys
import urllib.parse

root, backend = sys.argv[1:]

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.respond()

    def do_POST(self):
        self.respond()

    def log_message(self, *_args):
        pass

    def respond(self):
        if self.path.startswith('/redirect.git'):
            self.send_response(302)
            self.send_header('Location', '/forbidden.git/info/refs')
            self.end_headers()
            return
        if self.path.startswith('/forbidden.git'):
            pathlib.Path(root, 'forwarded').write_text('unexpected forwarding')
        if self.headers.get('Authorization') != 'Bearer ' + pathlib.Path(root, 'token').read_text():
            self.send_response(401)
            self.end_headers()
            self.wfile.write(b'private-auth-failure')
            return
        path = urllib.parse.urlsplit(self.path)
        env = dict(os.environ, GIT_PROJECT_ROOT=root, GIT_HTTP_EXPORT_ALL='1',
                   PATH_INFO=path.path, QUERY_STRING=path.query,
                   REQUEST_METHOD=self.command, CONTENT_TYPE=self.headers.get('Content-Type', ''),
                   REMOTE_USER='fixture', HTTP_GIT_PROTOCOL=self.headers.get('Git-Protocol', ''))
        body = self.rfile.read(int(self.headers.get('Content-Length', '0')))
        result = subprocess.run([backend], input=body, stdout=subprocess.PIPE, env=env, check=True, timeout=10)
        headers, body = result.stdout.split(b'\r\n\r\n', 1)
        self.send_response(200)
        for header in headers.decode().split('\r\n'):
            key, value = header.split(':', 1)
            self.send_header(key, value.strip())
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

server = http.server.HTTPServer(('127.0.0.1', 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(root + '/cert.pem', root + '/key.pem')
server.socket = context.wrap_socket(server.socket, server_side=True)
print(server.server_address[1], flush=True)
server.serve_forever()
