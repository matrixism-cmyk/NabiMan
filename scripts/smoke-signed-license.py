#!/usr/bin/env python3
"""Exercise the offline issuer and actual license API in an isolated server."""
import datetime
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('NABIMAN_SMOKE_BINARY', ROOT / 'backend/target/debug/nabiman-server')).resolve()
ISSUER = ROOT / 'tools/license-issuer.py'


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


def run_issuer(*args):
    run = subprocess.run(['python3', str(ISSUER), *map(str, args)], capture_output=True, timeout=25)
    assert run.returncode == 0, 'Offline issuer failed'


with tempfile.TemporaryDirectory(prefix='nabiman-signed-license-') as tmp:
    directory = Path(tmp)
    private = directory / 'offline.pem'
    roots = directory / 'roots.json'
    run_issuer('generate-key', '--key-id', 'smoke-issuer', '--private-key', private, '--public-keys', roots)
    assert private.stat().st_mode & 0o777 == 0o600
    claims = {'license_id': 'isolated-license', 'product': 'nabiman', 'info': {
        'tier': 'pro', 'holder': 'Isolated test customer', 'issued_at': datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
        'expires_at': (datetime.datetime.now(datetime.timezone.utc).date() + datetime.timedelta(days=5)).isoformat(),
        'max_servers': 3, 'features': ['basic_monitoring', 'management']}}
    path = directory / 'claims.json'
    path.write_text(json.dumps(claims))
    output = directory / 'license.key'
    run_issuer('issue', '--key-id', 'smoke-issuer', '--private-key', private, '--claims', path, '--output', output)
    signed = output.read_text().strip()
    assert output.stat().st_mode & 0o777 == 0o600
    assert signed.startswith('NBL1.')
    duplicate = subprocess.run(['python3', str(ISSUER), 'generate-key', '--key-id', 'smoke-issuer', '--private-key', str(private), '--public-keys', str(roots)], capture_output=True)
    assert duplicate.returncode != 0, 'Issuer must refuse overwriting private keys'
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = {key: value for key, value in os.environ.items() if not key.startswith('NABIMAN_')}
    env.update(NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(directory / 'auth'),
               NABIMAN_MEC_DB=str(directory / 'mec.db'), NABIMAN_MEC_MODE='real',
               NABIMAN_MEC_KUBECONFIG=str(directory / 'missing-kubeconfig'),
               NABIMAN_LICENSE_PUBLIC_KEYS_FILE=str(roots),
               NABIMAN_STATIC=str(ROOT / 'frontend/build'), NABIMAN_PASSWORD='isolated-license-password-123')
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
            code, result = request(port, '/api/license/activate', token, {'key': signed})
            assert code == 200 and result['success'] and result['data']['active']
            assert result['data']['max_servers'] == 3 and result['data']['features'] == claims['info']['features']
            stored_path = directory / 'auth/license.json'
            stored = stored_path.read_bytes()
            assert set(json.loads(stored)) == {'signed_key'}
            assert stored_path.stat().st_mode & 0o777 == 0o600
            for bad in ['legacy-hmac', 'NBL1.invalid']:
                code, result = request(port, '/api/license/activate', token, {'key': bad})
                assert code == 400 and not result['success']
                assert stored_path.read_bytes() == stored, 'Rejected activation must preserve valid license'
            envelope = json.loads(bytes.fromhex(signed[5:]))
            envelope['payload'] += '00'
            tampered = 'NBL1.' + json.dumps(envelope).encode().hex()
            code, _ = request(port, '/api/license/activate', token, {'key': tampered})
            assert code == 400
            for value in [json.dumps({'tier': 'enterprise', 'features': ['unlimited_servers']}), json.dumps({'signed_key': tampered})]:
                stored_path.write_text(value)
                code, result = request(port, '/api/license', token)
                assert code == 200 and not result['data']['active'] and not result['data']['is_trial']
                assert result['data']['validation_state'] == 'invalid_license'
                code, result = request(port, '/api/license/features', token)
                assert code == 200 and result['data'] == ['basic_monitoring']
            stored_path.write_bytes(stored)
            root_bytes = roots.read_bytes()
            roots.unlink()
            code, result = request(port, '/api/license', token)
            assert code == 200 and not result['data']['active']
            assert result['data']['validation_state'] == 'verification_unavailable'
            code, _ = request(port, '/api/license/activate', token, {'key': signed})
            assert code == 503
            roots.write_bytes(root_bytes)
            code, result = request(port, '/api/license', token)
            assert code == 200 and result['data']['active'], 'Restored verification root must recover without restart'
            # Explicit expiry and public-key overlap/retirement exercise actual OpenSSL signatures.
            claims['info']['issued_at'] = (datetime.datetime.now(datetime.timezone.utc).date() - datetime.timedelta(days=10)).isoformat()
            claims['info']['expires_at'] = (datetime.datetime.now(datetime.timezone.utc).date() - datetime.timedelta(days=1)).isoformat()
            path.write_text(json.dumps(claims))
            expired_path = directory / 'expired.key'
            run_issuer('issue', '--key-id', 'smoke-issuer', '--private-key', private, '--claims', path, '--output', expired_path)
            code, result = request(port, '/api/license/activate', token, {'key': expired_path.read_text()})
            assert code == 400 and not result['success'] and stored_path.read_bytes() == stored
            stored_path.write_text(json.dumps({'signed_key': expired_path.read_text().strip()}))
            code, result = request(port, '/api/license', token)
            assert not result['data']['active'] and result['data']['validation_state'] == 'license_expired'
            stored_path.write_bytes(stored)
            run_issuer('generate-key', '--key-id', 'next-issuer', '--private-key', directory / 'next.pem', '--public-keys', roots)
            assert request(port, '/api/license', token)[1]['data']['active']
            remaining = json.loads(roots.read_text())
            remaining['keys'] = [k for k in remaining['keys'] if k['key_id'] == 'next-issuer']
            roots.write_text(json.dumps(remaining))
            result = request(port, '/api/license', token)[1]['data']
            assert not result['active'] and result['validation_state'] == 'invalid_license'
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
print('PASS: offline Ed25519 issuer, signed API activation, tampering, missing roots, expiry and rotation.')
