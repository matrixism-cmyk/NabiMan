#!/usr/bin/env python3
"""Verify conservative managed-candidate cleanup in an isolated mock server and browser."""
import json
import os
from pathlib import Path
import signal
import socket
import sqlite3
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

with tempfile.TemporaryDirectory(prefix='nabiman-candidate-cleanup-') as tmp:
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
            tenant_id = 'access-fixture'
            code, tenant = request(port, '/api/mec/v1/tenants', token, {
                'tenant_id': tenant_id, 'display_name': 'Access fixture', 'task_name': None, 'contact_email': None,
                'allocation': {'type': 'shared', 'node': None, 'gpu_label': None, 'tier': None},
                'quota': {'cpu_requests': '1', 'cpu_limits': '2', 'memory_requests': '1Gi', 'memory_limits': '2Gi', 'gpu': 0, 'storage': '1Gi', 'pods': 10, 'pvcs': 2, 'lb_services': 1}, 'options': {}})
            assert code == 201, 'Isolated tenant creation failed'
            code, result = request(port, '/api/users', token, {'username': 'scope-manager', 'password': 'isolated-manager-password-123', 'role': 'viewer'})
            assert code == 200 and result['success']
            members = request(port, '/api/users', token)[1]['data']
            local_id = next(member['id'] for member in members if member['username'] == 'identity-fixture')
            manager_id = next(member['id'] for member in members if member['username'] == 'scope-manager')
            for member_id, role in [(local_id, 'viewer'), (manager_id, 'admin')]:
                code, result = request(port, f'/api/users/{member_id}/review', token, {'status': 'active', 'memberships': [{'kind': 'tenant', 'scope_id': tenant_id, 'role': role, 'enabled': True}]})
                assert code == 200 and result['success']
            code, _ = request(port, f'/api/users/{local_id}/external-identities/rancher', token, {'realm': 'mock://rancher', 'remote_user_id': remote['data']['id'], 'reason': 'Verified fixture owner'})
            assert code == 201
            # Inject only in this temporary DB: the provider succeeds but its
            # confirmation audit cannot commit, producing a real unknown intent.
            with sqlite3.connect(directory / 'mec.db') as database:
                database.execute("CREATE TRIGGER reject_confirm BEFORE INSERT ON audit_logs WHEN NEW.action LIKE '%_managed_confirmed' BEGIN SELECT RAISE(ABORT,'isolated browser audit fault'); END;")
            code, _ = request(port, f'/api/mec/v1/tenants/{tenant_id}/managed-memberships', token, {'expected_realm': 'mock://rancher', 'expected_remote_user_id': remote['data']['id'], 'local_user_id': local_id, 'role_template': 'read-only', 'automatic_revocation': True, 'acknowledge_project_role_scope': True, 'reason': 'Explicitly approved fixture access and cleanup'})
            assert code == 502, "Managed audit fault must preserve an uncertain provider result"
            with sqlite3.connect(directory / 'mec.db') as database:
                operation_id, state, candidate = database.execute("SELECT id,state,binding_id FROM membership_operations WHERE tenant_id=?", (tenant_id,)).fetchone()
                assert state == 'unknown_result' and candidate
                assert database.execute('SELECT COUNT(*) FROM owned_project_bindings').fetchone()[0] == 0
                database.execute('DROP TRIGGER reject_confirm')
            code, manager_login = request(port, '/api/auth/login', body={'username': 'scope-manager', 'password': 'isolated-manager-password-123'})
            assert code == 200 and manager_login['success']
            fixture = {'token': manager_login['data']['token'], 'refresh': manager_login['data']['refresh_token'], 'remote_id': remote['data']['id'], 'local_id': local_id, 'tenant_id': tenant_id, 'admin_token': token}
            child = subprocess.Popen(['node', str(ROOT / 'frontend/scripts/smoke-managed-candidate-cleanup-browser.js'), f'http://127.0.0.1:{port}'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
            try:
                output, diagnostic = child.communicate(json.dumps(fixture), timeout=80)
                assert child.returncode == 0, 'Actual browser access inspection smoke failed: ' + diagnostic.strip()
                with sqlite3.connect(directory / 'mec.db') as database:
                    assert database.execute('SELECT state FROM membership_operations WHERE id=?', (operation_id,)).fetchone()[0] == 'completed'
                    assert database.execute('SELECT binding_id FROM owned_project_bindings WHERE operation_id=?', (operation_id,)).fetchone()[0] == candidate
                    assert database.execute("SELECT COUNT(*) FROM owned_project_bindings WHERE state='active'").fetchone()[0] == 0
                    assert database.execute('SELECT COUNT(*) FROM requested_managed_grants').fetchone()[0] == 1
                    assert database.execute("SELECT COUNT(*) FROM membership_operations WHERE operation='create'").fetchone()[0] == 1
                    assert database.execute("SELECT COUNT(*) FROM membership_operations WHERE operation='delete'").fetchone()[0] == 1
                    assert database.execute("SELECT COUNT(*) FROM audit_logs WHERE user='system:managed-candidate-confirmation'").fetchone()[0] >= 1
                    assert database.execute('SELECT COUNT(*) FROM managed_project_bindings').fetchone()[0] == 1
                print(output.strip())
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=5)
                raise AssertionError('Browser access inspection smoke timeout') from None
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
