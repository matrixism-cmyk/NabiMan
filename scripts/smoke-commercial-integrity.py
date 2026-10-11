#!/usr/bin/env python3
"""Verify real-mode failures and durable startup using an isolated built server."""
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


def request(port, path, token=None, body=None):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = Request(f'http://127.0.0.1:{port}{path}',
                  None if body is None else json.dumps(body).encode(), headers)
    try:
        with urlopen(req, timeout=5) as response:
            return response.status, json.load(response)
    except HTTPError as error:
        return error.code, json.loads(error.read())


with tempfile.TemporaryDirectory(prefix='nabiman-integrity-') as temporary:
    directory = pathlib.Path(temporary)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    # Never inherit live connector credentials into the isolated test process.
    env = {key: value for key, value in os.environ.items() if not key.startswith('NABIMAN_')}
    env.update(NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(directory / 'auth'),
               NABIMAN_MEC_DB=str(directory / 'mec.db'), NABIMAN_MEC_MODE='real',
               NABIMAN_MEC_KUBECONFIG=str(directory / 'missing-kubeconfig'),
               NABIMAN_STATIC=str(ROOT / 'frontend/build'),
               NABIMAN_PASSWORD='isolated-integrity-password-123')
    for overrides in [{'NABIMAN_MEC_DB': str(directory)},
                      {'NABIMAN_MEC_DB': ':memory:'},
                      {'NABIMAN_MEC_MODE': 'typo'}]:
        run = subprocess.run([str(BINARY)], env={**env, **overrides}, cwd=directory,
                             capture_output=True, timeout=15)
        assert run.returncode != 0, 'Invalid startup must fail'
        assert not (directory / 'auth').exists(), 'Failure must precede auth initialization'
    for mode in ['real', 'auto']:
        with (directory / f'{mode}.log').open('w') as log:
            server = subprocess.Popen([str(BINARY)], env={**env, 'NABIMAN_MEC_MODE': mode},
                                      cwd=directory, stdout=log, stderr=log)
            try:
                for attempt in range(100):
                    assert server.poll() is None, 'Isolated server exited during startup'
                    try:
                        status, login = request(port, '/api/auth/login', body={
                            'username': 'admin', 'password': env['NABIMAN_PASSWORD']})
                        assert status == 200 and login['success']
                        token = login['data']['token']
                        break
                    except OSError:
                        time.sleep(0.1)
                else:
                    raise AssertionError('Startup timeout')
                status, health = request(port, '/api/mec/v1/health/summary', token)
                assert status == 200
                for source in ['kubernetes', 'rancher', 'axgate', 'harbor']:
                    entry = health['data'][source]
                    assert entry['connected'] is False and entry['error'], source
                    assert not entry['endpoint'].startswith('mock://'), source
                for path, body in [('/nodes', None), ('/firewall/nat-rules', None),
                                   ('/storage/harbor/projects', None),
                                   ('/nodes/nonexistent/cordon', {}),
                                   ('/storage/harbor/projects', {'name': 'isolated-project', 'public': False})]:
                    status, result = request(port, '/api/mec/v1' + path, token, body)
                    assert status == 503, (path, status)
                    assert result['data'] is None and result['error']['code'] == 'UNAVAILABLE', path
                status, _ = request(port, '/api/mec/v1/tenants', token, {
                    'tenant_id': 'unavailable-inspection-' + mode, 'display_name': 'Isolated test',
                    'allocation': {'type': 'shared'}, 'options': {},
                    'quota': {'cpu_requests': '1', 'cpu_limits': '1', 'memory_requests': '1Gi',
                        'memory_limits': '1Gi', 'gpu': 0, 'storage': '1Gi', 'pods': 1,
                        'pvcs': 1, 'lb_services': 1}})
                assert status == 503
                status, jobs = request(port, '/api/mec/v1/jobs', token)
                assert status == 200
                job_id = jobs['data'][0]['id']
                status, evidence = request(port, f'/api/mec/v1/jobs/{job_id}/recovery', token)
                assert status == 200
                evidence = evidence['data']
                assert evidence['configured_mode'] == mode and not evidence['automatic_replay']
                assert all(o['state'] == 'unknown' and o['error_code'] == 'UNAVAILABLE' for o in evidence['observations'])
                print(f'PASS {mode}: disconnected health, inventory and mutations; no demo fallback')
                print(f'PASS {mode}: failed tenant job recovery distinguishes unavailable resources from absence')
            finally:
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)
    corrupt_path = directory / 'corrupt.db'
    with sqlite3.connect(corrupt_path) as db:
        db.execute('CREATE TABLE jobs(id TEXT PRIMARY KEY,kind TEXT,status TEXT,spec TEXT,steps TEXT,result TEXT,error TEXT,created_by TEXT,started_at TEXT,completed_at TEXT)')
        db.execute('INSERT INTO jobs(id,kind,status,steps,created_by,started_at) VALUES(?,?,?,?,?,?)',
            ('corrupt-running', '{"kind":"tenant_delete","tenant_id":"test"}',
             '{"state":"running"}', 'bad checkpoint', 'admin', '2026-10-05T00:00:00Z'))
        db.commit()
    corrupt_auth = directory / 'corrupt-auth'
    run = subprocess.run([str(BINARY)], env={**env, 'NABIMAN_MEC_DB': str(corrupt_path),
                         'NABIMAN_DATA_DIR': str(corrupt_auth)}, cwd=directory,
                         capture_output=True, timeout=15)
    assert run.returncode != 0 and b'Cannot reconcile interrupted MEC jobs' in run.stderr
    assert not corrupt_auth.exists(), 'Recovery failure must precede auth initialization'
    print('PASS corrupt active checkpoint: startup aborts before auth side effects')
    assert (directory / 'mec.db').is_file(), 'Durable DB missing'
    print('PASS invalid mode and non-durable/unopenable DB abort before side effects')
    shares_auth = directory / 'corrupt-share-auth'
    shares_auth.mkdir()
    shares_path = shares_auth / 'terminal_shares.json'
    shares_path.write_text('invalid-share-registry')
    run = subprocess.run([str(BINARY)], env={**env, 'NABIMAN_DATA_DIR': str(shares_auth)},
                         cwd=directory, capture_output=True, timeout=15)
    assert run.returncode != 0, 'Corrupt share registry must abort startup'
    assert shares_path.read_text() == 'invalid-share-registry', 'Corruption must not be replaced by an empty registry'
    assert sorted(path.name for path in shares_auth.iterdir()) == ['terminal_shares.json', 'terminal_shares.lock'], 'Share validation failure must precede auth side effects'
    print('PASS corrupt share registry: startup aborts before auth side effects; no empty fallback')

    sessions_auth = directory / 'corrupt-terminal-auth'
    sessions_auth.mkdir()
    sessions_path = sessions_auth / 'terminal_sessions.json'
    sessions_path.write_text('invalid-terminal-registry')
    run = subprocess.run([str(BINARY)], env={**env, 'NABIMAN_DATA_DIR': str(sessions_auth)},
                         cwd=directory, capture_output=True, timeout=15)
    assert run.returncode != 0, 'Corrupt terminal registry must abort startup'
    assert sessions_path.read_text() == 'invalid-terminal-registry'
    assert sorted(path.name for path in sessions_auth.iterdir()) == ['terminal_sessions.json', 'terminal_sessions.lock', 'terminal_shares.json', 'terminal_shares.lock'], 'Terminal validation failure must precede auth side effects'
    print('PASS corrupt terminal registry: startup aborts before auth side effects; original corruption preserved')
    preferences_auth = directory / 'corrupt-preferences-auth'
    preferences_auth.mkdir()
    preferences_path = preferences_auth / 'terminal_settings.json'
    preferences_path.write_text('invalid-preferences-registry')
    run = subprocess.run([str(BINARY)], env={**env, 'NABIMAN_DATA_DIR': str(preferences_auth)}, cwd=directory, capture_output=True, timeout=15)
    assert run.returncode != 0 and preferences_path.read_text() == 'invalid-preferences-registry'
    assert not (preferences_auth / 'users.json').exists() and not (preferences_auth / 'jwt_secret').exists(), 'Preference validation failure initialized authentication'
    print('PASS corrupt preferences: startup aborts before auth initialization without replacing corruption by defaults')
