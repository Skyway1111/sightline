"""#41 catalog entry `http-in-loop`, and the loopback server its bench talks
to.

n=100: real loopback sockets; connection setup dominates identically at any
scale.
"""

import threading
import urllib.request


def http_slow(url, n):
    total = 0
    for _ in range(n):
        total += len(urllib.request.urlopen(url).read())  # sightline-ok: 41
    return total


def http_fast(url, n):
    import http.client

    host, port = url.split("//")[1].split(":")
    conn = http.client.HTTPConnection(host, int(port.rstrip("/")))
    total = 0
    for _ in range(n):
        conn.request("GET", "/")
        total += len(conn.getresponse().read())
    conn.close()
    return total


def _http_setup(n):
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

    class Handler(BaseHTTPRequestHandler):
        # read by the base class at request time  # sightline-ok: 32
        protocol_version = "HTTP/1.1"

        def do_GET(self):
            body = b"ok"
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return (f"http://127.0.0.1:{server.server_address[1]}/", n)
