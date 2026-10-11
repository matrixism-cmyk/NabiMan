#!/usr/bin/env python3
"""Isolated shared terminal revocation, expiry and immutable-owner authorization."""
import json
import os
from pathlib import Path
import socket
import signal
import shutil
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('NABIMAN_SMOKE_BINARY', ROOT / 'backend/target/debug/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nabiman-shared-revocation-') as tmp:
    directory = Path(tmp)
    # Serve one completed asset snapshot throughout all guest navigations/restarts.
    shutil.copytree(ROOT / 'frontend/build', directory / 'static')
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = {k: v for k, v in os.environ.items() if not k.startswith('NABIMAN_')}
    (directory / 'tmux').mkdir(mode=0o700)
    env['TMUX_TMPDIR'] = str(directory / 'tmux')
    env.update(NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(directory / 'auth'),
               NABIMAN_MEC_DB=str(directory / 'mec.db'), NABIMAN_MEC_MODE='mock',
               NABIMAN_STATIC=str(directory / 'static'), NABIMAN_PASSWORD='isolated-terminal-password-123')
    token = None

    def request(path, body=None, headers=None):
        sent = {'Content-Type': 'application/json'}
        if token:
            sent['Authorization'] = 'Bearer ' + token
        sent.update(headers or {})
        req = Request(f'http://127.0.0.1:{port}' + path, None if body is None else json.dumps(body).encode(), sent)
        try:
            response = urlopen(req, timeout=10)
        except HTTPError as error:
            response = error
        with response:
            request.headers = {k.lower(): v for k, v in response.headers.items()}
            data = response.read()
            try:
                result = json.loads(data)
            except ValueError:
                result = {}
            return response.status, result

    with (directory / 'server.log').open('w') as log:
        server = subprocess.Popen([str(BINARY)], env=env, cwd=directory, stdout=log, stderr=log)
        try:
            for _ in range(100):
                assert server.poll() is None, 'Isolated server exited'
                try:
                    code, login = request('/api/auth/login', {'username': 'admin', 'password': env['NABIMAN_PASSWORD']})
                    assert code == 200 and login['success']
                    token = login['data']['token']
                    break
                except OSError:
                    time.sleep(0.1)
            else:
                raise AssertionError('Startup timeout')
            code, created = request('/api/users', {'username': 'recovery', 'password': 'isolated-recovery-password-123', 'role': 'admin'})
            assert code == 200 and created['success']
            code, users = request('/api/users')
            owner_id = next(user['id'] for user in users['data'] if user['username'] == 'admin')
            code, recovery = request('/api/auth/login', {'username': 'recovery', 'password': 'isolated-recovery-password-123'})
            assert code == 200 and recovery['success']
            browser = subprocess.Popen(['node', str(ROOT / 'frontend/scripts/smoke-shared-revocation-browser.js'), f'http://127.0.0.1:{port}'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
            try:
                stdout, stderr = browser.communicate(json.dumps({'token': token, 'refresh': login['data']['refresh_token'], 'recovery_token': recovery['data']['token'], 'owner_id': owner_id, 'owner_password': env['NABIMAN_PASSWORD']}), timeout=130)
            except subprocess.TimeoutExpired:
                os.killpg(browser.pid, signal.SIGTERM)
                try:
                    browser.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(browser.pid, signal.SIGKILL)
                    browser.communicate(timeout=5)
                raise AssertionError('Isolated shared browser timeout; process group terminated')
            if browser.returncode:
                (directory / 'browser-error.log').write_text(stderr)
                print(stderr[-2500:])
            assert browser.returncode == 0, 'Actual shared terminal flow failed'
            print(stdout.strip())
            links = json.loads((directory / 'auth/terminal_shares.json').read_text())
            assert links and all(link['owner_id'] == owner_id for link in links.values())
            assert all(link['expires_unix'] > 0 for link in links.values())
            assert (directory / 'auth/terminal_shares.json').stat().st_mode & 0o777 == 0o600
            stored_users = json.loads((directory / 'auth/users.json').read_text())
            version = next(u['share_authorization_version'] for u in stored_users if u['id'] == owner_id)
            assert version and all(link['owner_authorization_version'] for link in links.values())
            stale = [link for link in links.values() if link['owner_authorization_version'] != version]
            fresh = [link for link in links.values() if link['owner_authorization_version'] == version]
            assert stale and fresh, 'Missing stale and freshly approved generations'
            server.terminate()
            server.wait(timeout=10)
            server = subprocess.Popen([str(BINARY)], env=env, cwd=directory, stdout=log, stderr=log)
            token = None
            for _ in range(100):
                assert server.poll() is None, 'Restart failed'
                try:
                    code, login = request('/api/auth/login', {'username': 'admin', 'password': env['NABIMAN_PASSWORD']})
                    if code == 200 and login.get('success'):
                        token = login['data']['token']
                        break
                except OSError:
                    pass
                time.sleep(0.1)
            else:
                raise AssertionError('Restart timeout')
            for link in stale:
                code, denied = request('/api/share/' + link['token'])
                assert not denied.get('success'), 'Restart resurrected old share'
            code, allowed = request('/api/share/' + fresh[0]['token'])
            assert code == 200 and allowed.get('success'), 'Restart lost freshly approved share'
            restored_users = json.loads((directory / 'auth/users.json').read_text())
            assert next(u['share_authorization_version'] for u in restored_users if u['id'] == owner_id) == version
            print('PASS: durable owner generation survives restart; stale shares remain denied and a freshly approved share remains usable.')
            import re
            audit = [json.loads(line) for line in (directory / 'auth/audit.jsonl').read_text().splitlines()]
            share_paths = [entry['path'] for entry in audit if entry['path'].startswith('/api/share/') or entry['path'].startswith('/api/terminal/share-links/')]
            assert share_paths and all('[cap:' in path for path in share_paths)
            assert all(not re.search(r'/(?:share|share-links)/[a-f0-9]{64}(?:/|$)', path) for path in share_paths), 'Bearer capability leaked in audit path'
            assert all(token not in '\n'.join(share_paths) for token in links), 'Capability leaked in audit path'
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
            # Only the sessions recorded by this isolated fixture; never kill a tmux server.
            registry = directory / 'auth/terminal_sessions.json'
            if registry.exists():
                for value in json.loads(registry.read_text()).values():
                    name = value.get('tmux_name', '')
                    if name.startswith('nb_admin_'):
                        subprocess.run(['tmux', 'kill-session', '-t', name], env=env, capture_output=True)
print('PASS: durable private registry, finite shared capabilities and real live connection revocation.')
