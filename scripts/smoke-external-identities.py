#!/usr/bin/env python3
"""Run identity linking in an isolated mock server and real browser."""
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen
ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('NABIMAN_SMOKE_BINARY', ROOT / 'backend/target/debug/nabiman-server')).resolve()

def request(port, path, token=None, body=None):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = Request(f'http://127.0.0.1:{port}{path}', None if body is None else json.dumps(body).encode(), headers)
    try:
        with urlopen(req, timeout=5) as response:
            return response.status, json.load(response)
    except HTTPError as error:
        return error.code, json.loads(error.read())

with tempfile.TemporaryDirectory(prefix='nabiman-identity-smoke-') as tmp:
    directory = Path(tmp)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = {key: value for key, value in os.environ.items() if not key.startswith('NABIMAN_')}
    env.update(NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(directory / 'auth'),
               NABIMAN_MEC_DB=str(directory / 'mec.db'), NABIMAN_MEC_MODE='mock',
               NABIMAN_STATIC=str(ROOT / 'frontend/build'), NABIMAN_PASSWORD='identity-smoke-password-123')
    with (directory / 'server.log').open('w') as log:
        server = subprocess.Popen([str(BINARY)], env=env, cwd=directory, stdout=log, stderr=log)
        try:
            for attempt in range(100):
                assert server.poll() is None, 'Isolated server exited'
                try:
                    code, login = request(port, '/api/auth/login', body={'username': 'admin', 'password': env['NABIMAN_PASSWORD']})
                    assert code == 200 and login['success']
                    token = login['data']['token']
                    break
                except OSError:
                    time.sleep(0.1)
            else:
                raise AssertionError('Startup timeout')
            code, result = request(port, '/api/users', token, {'username': 'identity-fixture', 'password': 'isolated-member-password-123', 'role': 'viewer'})
            assert code == 200 and result['success'], 'Fixture member creation failed'
            code, remote = request(port, '/api/mec/v1/users', token, {'username': 'external-fixture', 'password': 'isolated-provider-password-123'})
            assert code == 201 and remote['data']['id'], 'Fixture external account creation failed'
            fixture = {'token': token, 'refresh': login['data']['refresh_token'], 'remote_id': remote['data']['id']}
            child = subprocess.Popen(['node', str(ROOT / 'frontend/scripts/smoke-external-identities-browser.js'), f'http://127.0.0.1:{port}'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
            try:
                output, _ = child.communicate(json.dumps(fixture), timeout=75)
                assert child.returncode == 0, 'Actual browser identity smoke failed'
                print(output.strip())
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=5)
                raise AssertionError('Browser identity smoke timeout') from None
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
