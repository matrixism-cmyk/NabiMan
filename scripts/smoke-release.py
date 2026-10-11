#!/usr/bin/env python3
"""Verify that a release archive can serve the UI and authenticated API."""

import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
from urllib.error import URLError
from urllib.request import Request, urlopen


def run(archive: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="nabiman-release-smoke-") as root:
        base = Path(root)
        import importlib.util
        spec = importlib.util.spec_from_file_location("verifier", Path(__file__).with_name("verify-release.py"))
        verifier = importlib.util.module_from_spec(spec); spec.loader.exec_module(verifier)
        verifier.verify(archive)
        with tarfile.open(archive, "r:gz") as package:
            members = package.getmembers()
            if not all(m.name == "nabiman" or m.name.startswith("nabiman/") for m in members):
                raise RuntimeError("Archive contains a path outside nabiman/")
            if any(m.issym() or m.islnk() for m in members):
                raise RuntimeError("Archive contains a link")
            package.extractall(base, filter="data")

        binary = base / "nabiman" / "nabiman-server"
        helper = base / "nabiman" / "nabiman-privileged-helper"
        if not helper.is_file() or not os.access(helper, os.X_OK):
            raise RuntimeError("Archive is missing the executable privileged helper")
        for name in ["nabiman-privileged-helper.service", "nabiman-api-separated.service.example", "privileged-services.example.json"]:
            if not (base / "nabiman" / "packaging" / name).is_file():
                raise RuntimeError("Archive is missing helper deployment templates")
        recovery = base / "nabiman" / "nabiman-recovery"
        if not recovery.is_file() or not (base / "nabiman" / "management-recovery.py").is_file():
            raise RuntimeError("Archive is missing offline recovery tools")
        subprocess.run([str(recovery), "--help"], check=True, capture_output=True, timeout=10)
        static = base / "nabiman" / "static"
        if not binary.is_file() or not (static / "index.html").is_file():
            raise RuntimeError("Archive is missing the server or web interface")

        data = base / "data"
        data.mkdir()
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]

        password = "local-release-test-password"
        env = os.environ.copy()
        env.update({
            "NABIMAN_DATA_DIR": str(data),
            "NABIMAN_MEC_DB": str(data / "mec.db"),
            "NABIMAN_MEC_MODE": "mock",
            "NABIMAN_STATIC": str(static),
            "NABIMAN_PORT": str(port),
            "NABIMAN_PASSWORD": password,
        })
        process = subprocess.Popen([str(binary)], env=env,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        try:
            origin = f"http://127.0.0.1:{port}"
            for _ in range(80):
                if process.poll() is not None:
                    raise RuntimeError(f"Server exited: {process.stderr.read()[-1500:]}")
                try:
                    with urlopen(origin + "/", timeout=1) as response:
                        index = response.read().decode()
                    break
                except (URLError, TimeoutError):
                    time.sleep(0.25)
            else:
                raise RuntimeError("Server did not become ready")
            if "<html" not in index.lower():
                raise RuntimeError("Web interface did not render")
            assets = re.findall(r'(?:src|href)="(/static/(?:js|css)/[^" ]+)"', index)
            if not assets:
                raise RuntimeError("Web interface has no built assets")
            for asset in assets:
                with urlopen(origin + asset, timeout=5) as response:
                    body = response.read()
                if not body or body.lstrip().lower().startswith(b"<!doctype html"):
                    raise RuntimeError(f"Built asset did not load: {asset}")

            # The current deployment deliberately uses an HTTP/1-only backend listener.
            # A single HTTP/2 preface must be rejected, not negotiated into the h2 stack.
            # This is a protocol check, not an empty-DATA-frame flood or a proxy audit.
            with socket.create_connection(("127.0.0.1", port), timeout=5) as probe:
                probe.sendall(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
                rejected = probe.recv(1024)
            if not rejected.startswith(b"HTTP/1.1 400"):
                raise RuntimeError("Backend listener unexpectedly accepted HTTP/2 or failed to reject its preface")

            from http_framing import verify
            print(json.dumps({'http_framing': verify(port, password)}))

            login = Request(origin + "/api/auth/login",
                data=json.dumps({"username": "admin", "password": password}).encode(),
                headers={"Content-Type": "application/json"}, method="POST")
            with urlopen(login, timeout=5) as response:
                result = json.load(response)
            if not result.get("success") or not result.get("data", {}).get("token"):
                raise RuntimeError("Admin login failed")

            status = Request(origin + "/api/server/status", headers={
                "Authorization": "Bearer " + result["data"]["token"]})
            with urlopen(status, timeout=5) as response:
                protected = json.load(response)
            if not protected.get("success"):
                raise RuntimeError("Authenticated status API failed")
            identity = Request(origin + "/api/diagnostics/runtime", headers={
                "Authorization": "Bearer " + result["data"]["token"]})
            with urlopen(identity, timeout=5) as response:
                assert response.headers['Cache-Control'] == 'no-store'
                assert json.load(response)['data']['executable_sha256'] == hashlib.sha256(binary.read_bytes()).hexdigest()
            print("Release smoke passed: web UI/assets, HTTP/2 preface rejection, login, protected API")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("Usage: smoke-release.py ARCHIVE.tar.gz")
    run(Path(sys.argv[1]).resolve())
