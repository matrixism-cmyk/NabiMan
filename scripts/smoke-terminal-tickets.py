#!/usr/bin/env python3
"""Isolated account-ticket admission and browser revocation, without live credentials."""
import json
import os
from pathlib import Path
import socket
import signal
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('NABIMAN_SMOKE_BINARY', ROOT / 'backend/target/debug/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nabiman-terminal-tickets-') as tmp:
    directory = Path(tmp)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = {k: v for k, v in os.environ.items() if not k.startswith('NABIMAN_')}
    (directory / 'tmux').mkdir(mode=0o700)
    env['TMUX_TMPDIR'] = str(directory / 'tmux')
    env.update(NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(directory / 'auth'),
               NABIMAN_MEC_DB=str(directory / 'mec.db'), NABIMAN_MEC_MODE='mock',
               NABIMAN_STATIC=str(ROOT / 'frontend/build'), NABIMAN_PASSWORD='isolated-terminal-password-123')
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
            assert request('/api/terminal?token=' + token)[0] == 401, 'URL JWT must be rejected'
            assert request('/api/terminal')[0] == 400, 'Invalid WebSocket upgrade must not launch a shell'
            assert json.loads((directory / 'auth/terminal_sessions.json').read_text()) == {}, 'Invalid upgrade must leave the isolated registry empty'
            from ws_protocol import verify
            print(json.dumps({'websocket_protocol': verify(port, request)}))
            for query in ['token=secret', 'ticket=secret', 'password=secret', 'server_id=one&server_id=two']:
                assert request('/api/terminal/tickets', {'query': query})[0] == 400
            code, issued = request('/api/terminal/tickets', {'query': 'history=1'})
            assert code == 200 and issued['success']
            assert request.headers.get('cache-control') == 'no-store'
            ticket = issued['data']['ticket']
            assert len(ticket) == 64 and issued['data']['expires_in_seconds'] == 30
            assert request('/api/terminal?history=0&ticket=' + ticket)[0] == 401
            assert request('/api/terminal?history=1&ticket=' + ticket)[0] == 401, 'Mismatched admission must be consumed'
            code, issued = request('/api/terminal/tickets', {'query': 'history=1'})
            assert code == 200
            ticket = issued['data']['ticket']
            assert request('/api/terminal?history=1&ticket=' + ticket)[0] == 400
            assert request('/api/terminal?history=1&ticket=' + ticket)[0] == 401, 'Admission replay must fail'
            code, issued = request('/api/terminal/tickets', {'query': 'history=1'})
            assert code == 200
            expired = issued['data']['ticket']
            time.sleep(31)
            assert request('/api/terminal?history=1&ticket=' + expired)[0] == 401
            with urlopen(Request(f'http://127.0.0.1:{port}/api/mec/v1/dashboard/live/stream', headers={'Authorization': 'Bearer ' + token}), timeout=5) as stream:
                assert stream.status == 200 and stream.headers.get('Cache-Control') == 'no-store'
                assert stream.readline(), 'Authorized dashboard stream must produce data'
                assert request('/api/auth/logout', {})[0] == 200
                stream.read(1_000_000)  # HTTP EOF required; a still-open stream times out.
            print('PASS: actual dashboard HTTP stream reaches EOF after logout')
            code, login = request('/api/auth/login', {'username': 'admin', 'password': env['NABIMAN_PASSWORD']})
            assert code == 200 and login['success']
            token = login['data']['token']
            code, issued = request('/api/terminal/tickets', {'query': 'history=1'})
            assert code == 200
            revoked = issued['data']['ticket']
            download = directory / 'download-proof.txt'
            download.write_text('Isolated download proof\n')
            browser = subprocess.Popen(['node', str(ROOT / 'frontend/scripts/smoke-terminal-tickets-browser.js'), f'http://127.0.0.1:{port}'], stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
            try:
                stdout,stderr=browser.communicate(json.dumps({'token':token,'refresh':login['data']['refresh_token'],'download_path':str(download)}),timeout=100)
            except subprocess.TimeoutExpired:
                os.killpg(browser.pid,signal.SIGTERM)
                try:
                    browser.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(browser.pid,signal.SIGKILL)
                    browser.communicate(timeout=5)
                raise AssertionError('Isolated browser timeout; process group terminated')
            if browser.returncode:
                (directory / 'browser-error.log').write_text(stderr)
                print(stderr[-2500:])
            assert browser.returncode == 0, 'Actual browser ticket and revocation flow failed'
            print(stdout.strip())
            assert request('/api/terminal?history=1&ticket=' + revoked)[0] == 401
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
print('PASS: URL JWT rejected, upgrade guard, query binding, replay, 30-second expiry and session revocation.')
