#!/usr/bin/env python3
"""Inject a directory fsync failure into an isolated server; never touch production."""
import json
import os
import pathlib
import socket
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY', ROOT / 'backend/target/debug/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nabiman-user-publication-') as temporary:
    directory = pathlib.Path(temporary)
    auth = directory / 'auth'
    marker = directory / 'fail-sync'
    source = directory / 'inject.c'
    source.write_text(r'''
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <unistd.h>
#include <stdio.h>
#include <string.h>
int fsync(int fd) {
    int (*real_fsync)(int) = dlsym(RTLD_NEXT, "fsync");
    struct stat info;
    char reference[64], target[4096];
    const char *marker = getenv("NABIMAN_FIXTURE_SYNC_MARKER");
    const char *directory = getenv("NABIMAN_FIXTURE_AUTH_DIR");
    if (marker && directory && access(marker, F_OK) == 0 && fstat(fd, &info) == 0 && S_ISDIR(info.st_mode)) {
        snprintf(reference, sizeof(reference), "/proc/self/fd/%d", fd);
        ssize_t length = readlink(reference, target, sizeof(target)-1);
        if (length >= 0) { target[length] = 0; if (!strcmp(target, directory)) { errno = EIO; return -1; } }
    }
    return real_fsync(fd);
}
''')
    library = directory / 'inject.so'
    subprocess.run(['cc', '-shared', '-fPIC', str(source), '-o', str(library), '-ldl'], check=True, capture_output=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = dict(os.environ, NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(auth),
               NABIMAN_MEC_DB=str(directory / 'mec.db'), NABIMAN_MEC_MODE='mock',
               NABIMAN_STATIC_DIR=str(ROOT / 'frontend/build'),
               NABIMAN_PASSWORD='isolated-publication-password-123', LD_PRELOAD=str(library),
               NABIMAN_FIXTURE_SYNC_MARKER=str(marker), NABIMAN_FIXTURE_AUTH_DIR=str(auth))
    def request(path, data=None, token=None, method=None):
        headers = {'Content-Type': 'application/json'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        req = Request(f'http://127.0.0.1:{port}' + path, None if data is None else json.dumps(data).encode(), headers, method=method)
        try:
            response = urlopen(req, timeout=5)
        except HTTPError as error:
            response = error
        with response:
            return response.status, json.loads(response.read())
    def login(server):
        for _ in range(100):
            assert server.poll() is None, 'Fixture server exited before login'
            try:
                code, value = request('/api/auth/login', {'username': 'admin', 'password': env['NABIMAN_PASSWORD']})
                if code == 200 and value.get('success'):
                    return value['data']['token']
            except OSError:
                pass
            time.sleep(.1)
        raise AssertionError('Fixture login timeout')
    with (directory / 'server.log').open('w') as log:
        server = subprocess.Popen([str(BINARY)], env=env, cwd=directory, stdout=log, stderr=log)
        try:
            token = login(server)
            code, value = request('/api/users', {'username': 'recovery', 'password': 'isolated-recovery-password-123', 'role': 'admin'}, token)
            assert code == 200 and value['success']
            code, value = request('/api/users', token=token)
            owner = next(u['id'] for u in value['data'] if u['username'] == 'admin')
            before = json.loads((auth / 'users.json').read_text())
            old_version = next(u['share_authorization_version'] for u in before if u['id'] == owner)
            marker.touch()
            try:
                request('/api/users/' + owner, {'role': 'viewer'}, token, 'PUT')
            except OSError:
                pass
            else:
                raise AssertionError('Ambiguous account mutation returned an HTTP receipt')
            assert server.wait(timeout=10) == 78, 'Server did not fail closed on post-rename fsync error'
            observed = json.loads((auth / 'users.json').read_text())
            current = next(u for u in observed if u['id'] == owner)
            assert current['role'] == 'viewer' and current['share_authorization_version'] != old_version
            # A persistent fault must prevent the observed file from becoming live.
            server = subprocess.Popen([str(BINARY)], env=env, cwd=directory, stdout=log, stderr=log)
            assert server.wait(timeout=10) != 0, 'Restart accepted unflushable account state'
            marker.unlink()
            server = subprocess.Popen([str(BINARY)], env=env, cwd=directory, stdout=log, stderr=log)
            fresh_token = login(server)
            code, value = request('/api/users', token=fresh_token)
            assert code == 200 and len(value['data']) == 1 and value['data'][0]['role'] == 'viewer', 'Recovered viewer regained administrator visibility'
            code, value = request('/api/users', {'username': 'forbidden', 'password': 'isolated-forbidden-password-123', 'role': 'admin'}, fresh_token)
            assert code == 403, 'Recovered viewer regained administrator write access'
            recovered = json.loads((auth / 'users.json').read_text())
            assert next(u['share_authorization_version'] for u in recovered if u['id'] == owner) == current['share_authorization_version']
            print('PASS: actual HTTP mutation produces no receipt on ambiguous publication; process exits 78, persistent fsync fault blocks restart, recovered account retains revoked authority and new generation.')
        finally:
            if server.poll() is None:
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)
