"""Isolated production-mode smoke check. Synthetic credentials only; no OAuth exchange.

Usage: python deploy/smoke-production.py /path/to/melody-path-api frontend/dist
"""
import base64
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

binary, static = map(lambda value: str(Path(value).resolve()), sys.argv[1:3])
data = tempfile.mkdtemp(prefix="melody-production-smoke-")
with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
origin = "https://music.example.com"
env = {name: value for name, value in os.environ.items() if name.upper() in ("PATH", "SYSTEMROOT", "WINDIR", "TEMP", "TMP")}
env.update({"MELODYPATH_ENV": "production", "PUBLIC_BASE_URL": origin, "PORT": str(port),
            "PUBLIC_DEMO_EXPIRES_AT": "2026-09-21",
            "MELODYPATH_DATA_DIR": data, "MELODYPATH_STATIC_DIR": static,
            "OAUTH_TOKEN_STORE": "server_encrypted", "OAUTH_COOKIE_SECURE": "true",
            "OAUTH_TOKEN_ENCRYPTION_KEY": base64.b64encode(secrets.token_bytes(32)).decode(),
            "SPOTIFY_CLIENT_ID": "synthetic-client", "SPOTIFY_CLIENT_SECRET": "synthetic-secret",
            "GOOGLE_CLIENT_ID": "synthetic-client", "GOOGLE_CLIENT_SECRET": "synthetic-secret",
            "RUST_LOG": "off"})

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None

opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
def request(path, cookie="", method="GET", payload=None, source=origin):
    headers = {"Cookie": cookie, "Origin": source}
    if payload is not None:
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(f"http://127.0.0.1:{port}{path}", headers=headers, method=method,
                                 data=None if payload is None else json.dumps(payload).encode())
    try:
        return opener.open(req, timeout=5)
    except urllib.error.HTTPError as error:
        return error

def start(settings):
    return subprocess.Popen([binary], env=settings, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                            stderr=subprocess.PIPE, **({"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}))

# A bad production URL must fail before listening, never fall back to localhost.
bad = start({**env, "PUBLIC_BASE_URL": "http://127.0.0.1:3000"})
assert bad.wait(timeout=15) != 0
bad_key = start({**env, "OAUTH_TOKEN_ENCRYPTION_KEY": "invalid-synthetic-key"})
assert bad_key.wait(timeout=15) != 0
child = start(env)
try:
    for attempt in range(30):
        if child.poll() is not None:
            details = child.stderr.read().decode(errors="replace")
            details = details.replace(env["OAUTH_TOKEN_ENCRYPTION_KEY"], "[REDACTED]")
            raise AssertionError("Isolated synthetic process exited: " + details[:2000])
        try:
            if request("/health").status == 200:
                break
        except OSError:
            time.sleep(0.2)
    else:
        raise AssertionError("health did not become ready")
    for path in ("/", "/transfer", "/compare", "/versions"):
        assert request(path).status == 200
    boot = request("/api/session")
    assert json.load(request("/api/public-config")) == {"public_demo_expires_at": "2026-09-21"}
    value = boot.headers["Set-Cookie"]
    assert all(flag in value for flag in ("HttpOnly", "Secure", "SameSite=Lax"))
    cookie = value.split(";", 1)[0]
    for provider in ("spotify", "youtube"):
        response = request(f"/api/{provider}/authorize", cookie)
        assert response.status == 307
        location = urllib.parse.urlsplit(response.headers["Location"])
        query = urllib.parse.parse_qs(location.query)
        assert query["redirect_uri"] == [f"{origin}/api/{provider}/callback"]
        assert "Secure" in response.headers["Set-Cookie"]
        assert "secret" not in response.headers["Location"]
    assert request("/api/tasks").status == 401
    assert request("/api/settings", cookie, "PUT", {}, source=origin).status == 403
    assert request("/api/imports/preview", cookie, "POST", {}, source="https://evil.example").status == 403
    assert request("/api/does-not-exist", cookie).status == 404
    assert json.load(request("/api/spotify/me", cookie))["connected"] is False
    assert json.load(request("/api/youtube/me", cookie))["connected"] is False
    print("PASS: production static routes, health, exact public OAuth callbacks, Secure cookies, CSRF and fail-closed configuration")
finally:
    if child.poll() is None:
        child.send_signal(signal.SIGTERM)
        child.wait(timeout=15)
    if os.name != "nt":
        assert child.returncode == 0, "Linux SIGTERM must drain and exit successfully"
        print("PASS: Linux graceful SIGTERM")
    # Leave only this isolated synthetic temp directory for debugging; never touch user data.
