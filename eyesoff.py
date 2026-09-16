#!/usr/bin/env python3
import argparse
import base64
import hashlib
import http.client
import http.server
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

UPSTREAM = "api.anthropic.com"
OCR = Path(__file__).resolve().parent / "eyesoff-ocr"
HIDDEN = "[hidden by eyesoff]"
SCREENSHOT_REMOVED = "[screenshot removed by eyesoff: it could not be checked for secrets]"
HOP_HEADERS = {"host", "connection", "keep-alive", "transfer-encoding", "content-length", "proxy-connection", "upgrade", "te", "trailer"}
TEXT_KEYS = {"text", "content", "system"}
PREFIXES = (
    "sk_live_", "sk_test_", "rk_live_", "rk_test_", "whsec_", "ghp_", "gho_", "ghu_", "ghs_",
    "github_pat_", "xoxb-", "xoxp-", "AKIA", "sk-ant-", "sk-proj-", "glpat-", "npm_",
)
SAFE_PREFIXES = ("toolu_", "srvtoolu_", "msg_", "req_")
TOKEN = re.compile(r"[A-Za-z0-9_\-]{16,}")
ENV_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def looks_like_secret(word):
    if word.startswith(SAFE_PREFIXES):
        return False
    if word.startswith(PREFIXES):
        return True
    return (
        len(word) >= 20
        and sum(c.isdigit() for c in word) >= 3
        and any(c.isupper() for c in word)
        and any(c.islower() for c in word)
    )


def redact_text(text, stats):
    def replace(match):
        if not looks_like_secret(match.group()):
            return match.group()
        stats["strings"] += 1
        return HIDDEN

    return TOKEN.sub(replace, text)


checked_images = {}


def redact_image(data):
    digest = hashlib.sha256(data.encode()).digest()
    if digest not in checked_images:
        result = subprocess.run([OCR], input=base64.b64decode(data), capture_output=True, timeout=60)
        if result.returncode != 0:
            raise RuntimeError(result.stderr.decode(errors="replace").strip())
        checked_images[digest] = base64.b64encode(result.stdout).decode() if result.stdout else None
    return checked_images[digest]


def scrub(node, stats, key=None):
    if isinstance(node, dict):
        source = node.get("source")
        if node.get("type") == "image" and isinstance(source, dict) and source.get("type") == "base64":
            try:
                redacted = redact_image(source["data"])
            except Exception as error:
                print(f"eyesoff: {error}", file=sys.stderr, flush=True)
                stats["screenshots"] += 1
                return {"type": "text", "text": SCREENSHOT_REMOVED}
            if redacted:
                stats["screenshots"] += 1
                return {**node, "source": {"type": "base64", "media_type": "image/png", "data": redacted}}
            return node
        return {k: scrub(v, stats, k) for k, v in node.items()}
    if isinstance(node, list):
        return [scrub(v, stats, key) for v in node]
    if isinstance(node, str) and key in TEXT_KEYS:
        return redact_text(node, stats)
    return node


def scrub_body(raw):
    stats = {"strings": 0, "screenshots": 0}
    body = scrub(json.loads(raw), stats)
    return json.dumps(body, ensure_ascii=False, separators=(",", ":")).encode(), stats


class Proxy(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self, status, message):
        body = json.dumps({"type": "error", "error": {"type": "eyesoff_error", "message": message}}).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def forward(self):
        if "chunked" in self.headers.get("transfer-encoding", ""):
            return self.reply(411, "eyesoff needs a content-length on requests")
        size = int(self.headers.get("content-length") or 0)
        body = self.rfile.read(size) if size else None
        stats = {"strings": 0, "screenshots": 0}
        if body and "json" in self.headers.get("content-type", ""):
            try:
                body, stats = scrub_body(body)
            except ValueError:
                if self.path.startswith("/v1/messages"):
                    return self.reply(400, "eyesoff could not parse the request, so it was not sent")

        headers = {k: v for k, v in self.headers.items() if k.lower() not in HOP_HEADERS}
        try:
            upstream = http.client.HTTPSConnection(UPSTREAM, timeout=600)
            upstream.request(self.command, self.path, body=body, headers=headers)
            response = upstream.getresponse()
        except OSError as error:
            return self.reply(502, f"eyesoff could not reach {UPSTREAM}: {error}")

        hidden = ", ".join(f"{n} {what if n > 1 else what[:-1]}" for what, n in stats.items() if n)
        print(f"{time.strftime('%H:%M:%S')} {self.command} {self.path} {response.status}" + (f"  hid {hidden}" if hidden else ""), flush=True)

        self.send_response(response.status)
        for k, v in response.getheaders():
            if k.lower() not in HOP_HEADERS:
                self.send_header(k, v)
        self.send_header("connection", "close")
        self.end_headers()
        while chunk := response.read1(65536):
            self.wfile.write(chunk)
            self.wfile.flush()

    do_GET = do_POST = do_PUT = do_PATCH = do_DELETE = forward


def start(port):
    if not OCR.exists():
        sys.exit(f"eyesoff: {OCR.name} is missing, run `make` in {OCR.parent}")
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Proxy)
    server.daemon_threads = True
    print(f"eyesoff is listening on http://127.0.0.1:{port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


def read_clipboard():
    return subprocess.run(["pbpaste"], capture_output=True, text=True, check=True).stdout


def clear_clipboard():
    subprocess.run(["pbcopy"], input=b"", check=True)


def write_env(path, name, value):
    path = Path(path).resolve()
    lines = path.read_text().splitlines() if path.exists() else []
    entry = f"{name}={value}"
    if any(line.startswith(f"{name}=") for line in lines):
        lines = [entry if line.startswith(f"{name}=") else line for line in lines]
    else:
        lines.append(entry)
    mode = path.stat().st_mode & 0o777 if path.exists() else 0o600
    temp = path.with_name(f".{path.name}.eyesoff")
    with os.fdopen(os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, mode), "w") as f:
        f.write("\n".join(lines) + "\n")
    os.replace(temp, path)


def paste(name, env_file, command):
    if not ENV_NAME.fullmatch(name):
        sys.exit(f"eyesoff: {name!r} is not a valid variable name")
    if bool(env_file) == bool(command):
        sys.exit("eyesoff: pass either --env FILE or a command after --")
    value = read_clipboard().strip()
    if not value or any(c.isspace() for c in value):
        sys.exit("eyesoff: the clipboard is empty or has whitespace in it, copy just the secret")

    if env_file:
        write_env(env_file, name, value)
        where = env_file
    else:
        result = subprocess.run(command, input=value, text=True)
        if result.returncode != 0:
            sys.exit(f"eyesoff: `{command[0]}` failed with exit code {result.returncode}, clipboard left as is")
        where = command[0]

    clear_clipboard()
    print(f"{name} saved to {where} ({len(value)} chars), clipboard cleared")


def main():
    parser = argparse.ArgumentParser(prog="eyesoff", description="Keep secrets out of what your coding agent sends to the model.")
    commands = parser.add_subparsers(dest="cmd", required=True)

    start_cmd = commands.add_parser("start", help="run the redacting proxy")
    start_cmd.add_argument("--port", type=int, default=8787)

    paste_cmd = commands.add_parser("paste", help="save the clipboard into an env file or pipe it to a command")
    paste_cmd.add_argument("name")
    paste_cmd.add_argument("--env", dest="env_file", metavar="FILE")
    paste_cmd.add_argument("command", nargs="*")

    args = parser.parse_args()
    if args.cmd == "start":
        start(args.port)
    else:
        paste(args.name, args.env_file, args.command)


if __name__ == "__main__":
    main()
