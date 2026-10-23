"""Development server for the web client.

Serves web/dist and forwards the server's HTTP endpoints from the same origin,
so the page needs no CORS headers from them:

    /data/...           -> the client data endpoint   (default http://127.0.0.1:6903)
    /db/..., /wiki/...  -> the reference server       (default http://127.0.0.1:6904)
    /forum/...          -> the forum on the bots API  (default http://127.0.0.1:6902)

With it, the site config can use paths, which the client resolves against the
page: "data_url": "/data", "reference_url": "/", "forum_url": "/" (see
config.dev.json). WebSocket upgrades on those paths (the forum's) are tunnelled.

    python web/serve.py [--port 8080] [--data http://host:6903] [--reference ...] [--forum ...]
"""

import argparse
import functools
import http.server
import os
import socket
import threading
import urllib.error
import urllib.parse
import urllib.request

ROUTES = {}


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
    }

    def upstream(self):
        for prefix, target in ROUTES.items():
            if self.path == prefix or self.path.startswith(prefix + "/") or self.path.startswith(prefix + "?"):
                return target + self.path
        return None

    def forward(self, method):
        url = self.upstream()
        if url is None:
            return False
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else None
        headers = {
            name: value
            for name, value in self.headers.items()
            if name.lower() in ("accept", "content-type", "authorization", "range")
        }
        request = urllib.request.Request(url, data=body, headers=headers, method=method)
        try:
            response = urllib.request.urlopen(request, timeout=60)
            status, reply_headers, payload = response.status, response.headers, response.read()
        except urllib.error.HTTPError as error:
            status, reply_headers, payload = error.code, error.headers, error.read()
        except OSError as error:
            self.send_error(502, f"{url}: {error}")
            return True
        self.send_response(status)
        for name in ("Content-Type", "Content-Encoding", "Cache-Control", "Last-Modified", "ETag"):
            if reply_headers.get(name):
                self.send_header(name, reply_headers[name])
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        if method != "HEAD":
            self.wfile.write(payload)
        return True

    def tunnel(self):
        """Hands a WebSocket upgrade to the upstream as is, then copies bytes both ways."""
        url = self.upstream()
        if url is None or self.headers.get("Upgrade", "").lower() != "websocket":
            return False
        target = urllib.parse.urlsplit(url)
        try:
            upstream = socket.create_connection((target.hostname, target.port or 80), timeout=10)
        except OSError as error:
            self.send_error(502, f"{url}: {error}")
            return True
        upstream.settimeout(None)
        head = [f"GET {self.path} HTTP/1.1", f"Host: {target.netloc}"]
        head += [f"{name}: {value}" for name, value in self.headers.items() if name.lower() != "host"]
        # A browser cannot set headers on a WebSocket: the client sends its
        # forum token as `?token=`, and the server wants it as a header.
        token = urllib.parse.parse_qs(target.query).get("token")
        if token and "Authorization" not in self.headers:
            head.append(f"Authorization: Bearer {token[0]}")
        upstream.sendall(("\r\n".join(head) + "\r\n\r\n").encode("latin-1"))

        def pump(source, sink):
            try:
                while data := source.recv(65536):
                    sink.sendall(data)
            except OSError:
                pass
            for end in (source, sink):
                try:
                    end.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass

        back = threading.Thread(target=pump, args=(upstream, self.connection), daemon=True)
        back.start()
        pump(self.connection, upstream)
        back.join()
        upstream.close()
        self.close_connection = True
        return True

    def do_GET(self):
        if not self.tunnel() and not self.forward("GET"):
            super().do_GET()

    def do_HEAD(self):
        if not self.forward("HEAD"):
            super().do_HEAD()

    def do_POST(self):
        if not self.forward("POST"):
            self.send_error(405)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--port", type=int, default=8080)
    parser.add_argument("--data", default="http://127.0.0.1:6903")
    parser.add_argument("--reference", default="http://127.0.0.1:6904")
    parser.add_argument("--forum", default="http://127.0.0.1:6902")
    args = parser.parse_args()
    ROUTES.update({
        "/data": args.data.rstrip("/"),
        "/db": args.reference.rstrip("/"),
        "/wiki": args.reference.rstrip("/"),
        "/forum": args.forum.rstrip("/"),
    })
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "dist")
    handler = functools.partial(Handler, directory=root)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), handler)
    print(f"Serving {root} on http://127.0.0.1:{args.port}/")
    server.serve_forever()


if __name__ == "__main__":
    main()
