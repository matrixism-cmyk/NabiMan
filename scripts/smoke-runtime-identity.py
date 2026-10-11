#!/usr/bin/env python3
"""Real executable inode replacement and restart identity, isolated from production."""
import datetime, hashlib, json, os, pathlib, re, shutil, socket, subprocess, tempfile, time, uuid
from urllib.request import Request, urlopen
from urllib.error import HTTPError
ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY', ROOT/'backend/target/debug/nabiman-server'))
with tempfile.TemporaryDirectory(prefix='nb-runtime-') as temporary:
    directory = pathlib.Path(temporary)
    binary = directory/'server'; shutil.copy2(BINARY, binary)
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    (directory/'tmux').mkdir()
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
    base = f'http://127.0.0.1:{port}'
    env = dict(os.environ, NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(directory/'auth'),
        NABIMAN_MEC_DB=str(directory/'mec.db'), NABIMAN_MEC_MODE='mock',
        NABIMAN_STATIC=str(ROOT/'frontend/build'), NABIMAN_PASSWORD='fixture-runtime-password-123',
        NABIMAN_BUILD_ID='untrusted-label', TMUX_TMPDIR=str(directory/'tmux'))
    def call(path, token=None, body=None):
        headers = {'Content-Type':'application/json'}
        if token: headers['Authorization'] = 'Bearer '+token
        request = Request(base+path, None if body is None else json.dumps(body).encode(), headers)
        try: response = urlopen(request, timeout=10)
        except HTTPError as error: response = error
        with response: return response.status, dict(response.headers), json.load(response)
    def start(log):
        process = subprocess.Popen([str(binary)], env=env, cwd=directory, stdout=log, stderr=log)
        try:
            for _ in range(600):
                assert process.poll() is None
                try:
                    status, _, value = call('/api/auth/login', body={'username':'admin','password':env['NABIMAN_PASSWORD']})
                    if status == 200: return process, value['data']['token']
                except OSError: pass
                time.sleep(.1)
            raise AssertionError('Fixture startup timeout')
        except BaseException:
            stop(process); raise
    def stop(process):
        if process and process.poll() is None:
            process.terminate()
            try: process.wait(timeout=10)
            except subprocess.TimeoutExpired: process.kill(); process.wait(timeout=5)
    process = None
    with (directory/'log').open('w') as log:
        try:
            process, token = start(log)
            assert call('/api/diagnostics/runtime')[0] == 401
            status, headers, value = call('/api/diagnostics/runtime', token)
            assert status == 200 and headers.get('cache-control', headers.get('Cache-Control')) == 'no-store'
            first = value['data']; assert first['executable_sha256'] == digest
            uuid.UUID(first['instance_id']); datetime.datetime.fromisoformat(re.sub(r'(\.\d{6})\d+', r'\1', first['started_at']))
            replacement = directory/'replacement'; replacement.write_text('different bytes, never executed')
            os.replace(replacement, binary)
            assert hashlib.sha256(binary.read_bytes()).hexdigest() != digest
            assert call('/api/diagnostics/runtime?build=spoof', token)[2]['data'] == first
            stop(process); shutil.copy2(BINARY, binary)
            process, token = start(log)
            second = call('/api/diagnostics/runtime', token)[2]['data']
            assert second['executable_sha256'] == digest
            assert second['instance_id'] != first['instance_id'] and second['started_at'] != first['started_at']
            print('PASS: authenticated no-store identity matches running executable; pathname replacement cannot spoof hash; restart changes instance while preserving executable digest.')
        finally: stop(process)
