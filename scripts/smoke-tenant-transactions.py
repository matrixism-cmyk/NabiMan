#!/usr/bin/env python3
"""Exercise tenant/job commit failures over HTTP using isolated mock providers."""
import json
import os
import pathlib
import socket
import sqlite3
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY', ROOT / 'backend/target/debug/nabiman-server')).resolve()


def spec(tenant_id):
    return {'tenant_id': tenant_id, 'display_name': 'Transaction smoke',
            'allocation': {'type': 'shared'}, 'options': {},
            'quota': {'cpu_requests': '32', 'cpu_limits': '64', 'memory_requests': '64Gi',
                      'memory_limits': '128Gi', 'gpu': 2, 'storage': '500Gi',
                      'pods': 100, 'pvcs': 20, 'lb_services': 5}}


with tempfile.TemporaryDirectory(prefix='nabiman-tenant-transaction-') as temporary:
    directory = pathlib.Path(temporary)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = {k: v for k, v in os.environ.items() if not k.startswith('NABIMAN_')}
    env.update(NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(directory / 'auth'),
               NABIMAN_MEC_DB=str(directory / 'mec.db'), NABIMAN_MEC_MODE='mock',
               NABIMAN_STATIC=str(ROOT / 'frontend/build'),
               NABIMAN_PASSWORD='isolated-transaction-password-123')
    token = None

    def request(path, body=None, method=None, headers=None):
        sent = {'Content-Type': 'application/json'}
        if token:
            sent['Authorization'] = 'Bearer ' + token
        sent.update(headers or {})
        req = Request(f'http://127.0.0.1:{port}' + path,
                      None if body is None else json.dumps(body).encode(), sent, method=method)
        try:
            response = urlopen(req, timeout=15)
        except HTTPError as error:
            response = error
        with response:
            request.last_headers = {k.lower(): v for k, v in response.headers.items()}
            raw = response.read()
            return response.status, json.loads(raw) if raw else None

    with (directory / 'server.log').open('w') as log:
        server = subprocess.Popen([str(BINARY)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(100):
                assert server.poll() is None, 'Isolated server exited'
                try:
                    status, login = request('/api/auth/login', {'username': 'admin', 'password': env['NABIMAN_PASSWORD']})
                    assert status == 200
                    token = login['data']['token']
                    break
                except OSError:
                    time.sleep(0.1)
            else:
                raise AssertionError('Startup timeout')
            with sqlite3.connect(directory / 'mec.db') as db:
                # Abort only successful completion: the orchestrator must still
                # persist a failed job and expose reconciliation requirements.
                db.execute('''CREATE TRIGGER block_finish BEFORE UPDATE OF completed_at ON jobs
                    WHEN NEW.status = '{"state":"completed"}'
                    BEGIN SELECT RAISE(ABORT, 'injected completion failure'); END''')
                db.commit()
                status, _ = request('/api/mec/v1/tenants', spec('tx-create-fail'))
                assert status == 500, status
                state, project, user = db.execute("SELECT status,rancher_project_id,rancher_user_id FROM tenant_profiles WHERE id='tx-create-fail'").fetchone()
                assert json.loads(state)['state'] == 'failed' and project and user
                kind, state, creator = db.execute('SELECT kind,status,created_by FROM jobs').fetchone()
                assert json.loads(kind)['spec']['tenant_id'] == 'tx-create-fail'
                assert json.loads(state)['state'] == 'failed' and creator == 'admin'
                db.execute('DROP TRIGGER block_finish')
                db.commit()
                status, _ = request('/api/mec/v1/tenants', spec('tx-delete-fail'))
                assert status == 201, status
                db.execute('''CREATE TRIGGER block_finish BEFORE UPDATE OF completed_at ON jobs
                    WHEN NEW.status = '{"state":"completed"}'
                    BEGIN SELECT RAISE(ABORT, 'injected completion failure'); END''')
                db.commit()
                status, _ = request('/api/mec/v1/tenants/tx-delete-fail', method='DELETE',
                                    headers={'X-Confirm-Name': 'tx-delete-fail'})
                assert status == 500, status
                state, project, user = db.execute("SELECT status,rancher_project_id,rancher_user_id FROM tenant_profiles WHERE id='tx-delete-fail'").fetchone()
                assert json.loads(state)['state'] == 'failed' and project and user
                jobs = [json.loads(row[0]) for row in db.execute("SELECT status FROM jobs WHERE kind LIKE '%tenant_delete%'")]
                assert len(jobs) == 1 and jobs[0]['state'] == 'failed'
                db.execute('DROP TRIGGER block_finish')
                db.commit()
                status, result = request('/api/mec/v1/tenants', spec('tx-success'))
                assert status == 201, status
                initial = result['data']['initial_credentials']
                assert len(initial['password']) == 48 and initial['username'] == 'tx-success-dev'
                assert request.last_headers.get('cache-control') == 'no-store'
                status, record = request('/api/mec/v1/tenants/tx-success')
                assert status == 200 and 'initial_credentials' not in record['data']
                assert initial['password'] not in json.dumps(record)
                status, member = request('/api/mec/v1/tenants/tx-success/members',
                    {'username': 'isolated-new-member', 'create_if_missing': True})
                assert status == 201, status
                assert request.last_headers.get('cache-control') == 'no-store'
                secret = member['data']['initial_credentials']['password']
                assert len(secret) == 48 and secret != initial['password']
                assert member['data']['role_template'] == 'project-member'
                status, _ = request('/api/mec/v1/tenants/tx-success/members',
                    {'username': 'weak-member', 'password': 'changeme!', 'create_if_missing': True})
                assert status == 400
                status, _ = request('/api/mec/v1/tenants/tx-success/members',
                    {'username': 'invalid-role-member', 'role': 'admin', 'create_if_missing': True})
                assert status == 400
                user_id = member['data']['user_id']
                status, user = request('/api/mec/v1/users/' + user_id)
                assert status == 200 and user['data']['must_change_password']
                status, _ = request('/api/mec/v1/users/' + user_id + '/reset-password',
                    {'new_password': 'weak'}, headers={'X-Confirm-Name': user_id})
                assert status == 400
                assert initial['password'] not in '\n'.join(db.iterdump())
                assert secret not in '\n'.join(db.iterdump())
                print('PASS credentials: unique one-time secrets, no-store responses, no persisted passwords, weak input and arbitrary role rejection')
                status, _ = request('/api/mec/v1/tenants/tx-success', method='DELETE',
                                    headers={'X-Confirm-Name': 'tx-success'})
                assert status == 204, status
                assert db.execute("SELECT COUNT(*) FROM tenant_profiles WHERE id='tx-success'").fetchone()[0] == 0
            # A second live server must fail before auth side effects or recovery.
            second_auth = directory / 'second-auth'
            second = subprocess.run([str(BINARY)], cwd=directory,
                env={**env, 'NABIMAN_DATA_DIR': str(second_auth)}, capture_output=True, timeout=15)
            assert second.returncode != 0
            assert b'exclusive MEC database ownership' in second.stderr
            assert not second_auth.exists(), 'Second server must not initialize auth'
            with sqlite3.connect(directory / 'mec.db') as db:
                for name, state in [('orphan-pending', 'pending'), ('orphan-running', 'running')]:
                    kind = json.dumps({'kind': 'tenant_delete', 'tenant_id': 'tx-delete-fail'}, separators=(',', ':'))
                    steps = json.dumps([{'name': 'delete_namespace', 'status': 'running',
                        'started_at': '2026-10-05T00:00:00Z', 'completed_at': None,
                        'duration_ms': None, 'progress': None, 'message': None}])
                    db.execute('INSERT INTO jobs(id,kind,status,steps,result,created_by,started_at) VALUES(?,?,?,?,?,?,?)',
                        (name, kind, json.dumps({'state': state}, separators=(',', ':')), steps,
                         json.dumps({'known_resource': 'preserved'}), 'admin', '2026-10-05T00:00:00Z'))
                db.execute("UPDATE tenant_profiles SET status=? WHERE id='tx-create-fail'", ('{"state":"provisioning"}',))
                db.execute("UPDATE tenant_profiles SET status=? WHERE id='tx-delete-fail'", ('{"state":"terminating"}',))
                db.commit()
            server.kill()
            server.wait(timeout=10)
            server = subprocess.Popen([str(BINARY)], cwd=directory, env=env, stdout=log, stderr=log)
            token = None
            for _ in range(100):
                assert server.poll() is None, 'Restarted server exited'
                try:
                    status, login = request('/api/auth/login', {'username': 'admin', 'password': env['NABIMAN_PASSWORD']})
                    assert status == 200
                    token = login['data']['token']
                    break
                except OSError:
                    time.sleep(0.1)
            else:
                raise AssertionError('Restart timeout')
            for tenant_id in ['tx-create-fail', 'tx-delete-fail']:
                status, result = request('/api/mec/v1/tenants/' + tenant_id)
                assert status == 200, status
                tenant = result['data']
                assert tenant['status']['state'] == 'failed'
                assert tenant['rancher_project_id'] and tenant['rancher_user_id']
            for name in ['orphan-pending', 'orphan-running']:
                status, result = request('/api/mec/v1/jobs/' + name)
                assert status == 200
                assert result['data']['status']['state'] == 'unknown_result'
                assert result['data']['completed_at'] is None
                assert len(result['data']['steps']) == 1
                status, evidence = request('/api/mec/v1/jobs/' + name + '/recovery')
                assert status == 200
                evidence = evidence['data']
                assert evidence['configured_mode'] == 'mock' and not evidence['automatic_replay']
                assert evidence['manual_verification_required']
                assert all(o['state'] == 'absent' for o in evidence['observations'])
                req = Request(f'http://127.0.0.1:{port}/api/mec/v1/jobs/{name}/stream',
                              headers={'Authorization': 'Bearer ' + token})
                with urlopen(req, timeout=5) as response:
                    snapshot = response.read().decode()
                    assert 'event: snapshot' in snapshot and 'unknown_result' in snapshot
                    assert 'delete_namespace' in snapshot
            with sqlite3.connect(directory / 'mec.db') as db:
                assert json.loads(db.execute("SELECT result FROM jobs WHERE id='orphan-running'").fetchone()[0])['known_resource'] == 'preserved'
            browser = subprocess.run(['node', str(ROOT / 'frontend/scripts/smoke-job-recovery-browser.js'),
                f'http://127.0.0.1:{port}'], input=json.dumps({'password': env['NABIMAN_PASSWORD']}),
                text=True, capture_output=True, timeout=60)
            assert browser.returncode == 0, browser.stderr
            print(browser.stdout.strip())
            print('PASS ownership: concurrent server rejected before auth or recovery; SIGKILL restart exposes unknown outcomes and persisted SSE steps')
            print('PASS restart: failed creation/deletion resource IDs remain available through authenticated tenant API')
            print('PASS HTTP: completion failure retains failed creation with resource IDs; deletion preserves failed reconciliation state and resource IDs')
            print('PASS HTTP: successful tenant create/delete commits with completed jobs; failures remain failed jobs')
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
