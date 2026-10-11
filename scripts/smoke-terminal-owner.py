#!/usr/bin/env python3
"""Real account deletion/reuse and legacy terminal metadata, in an isolated tmux namespace."""
import json, os, pathlib, signal, socket, subprocess, tempfile, time
from urllib.request import Request, urlopen
ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY', ROOT/'backend/target/debug/nabiman-server'))
with tempfile.TemporaryDirectory(prefix='nabiman-terminal-owner-') as temporary:
    directory = pathlib.Path(temporary); (directory/'tmux').mkdir()
    with socket.socket() as sock: sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
    env = dict(os.environ, NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(directory/'auth'), NABIMAN_MEC_DB=str(directory/'mec.db'), NABIMAN_MEC_MODE='mock', NABIMAN_STATIC=str(ROOT/'frontend/build'), NABIMAN_PASSWORD='isolated-identity-password-123', TMUX_TMPDIR=str(directory/'tmux'))
    base = f'http://127.0.0.1:{port}'
    def login(server):
        for _ in range(100):
            assert server.poll() is None, 'Fixture server exited'
            try:
                req = Request(base+'/api/auth/login', json.dumps({'username':'admin','password':env['NABIMAN_PASSWORD']}).encode(), {'Content-Type':'application/json'})
                with urlopen(req, timeout=10) as response:
                    value = json.load(response)
                    if value.get('success'): return value['data']['token']
            except OSError: pass
            time.sleep(.1)
        raise AssertionError('Fixture startup timeout')
    def browser(token, resume=None):
        run = subprocess.Popen(['node', str(ROOT/'frontend/scripts/smoke-terminal-owner.js')], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=ROOT/'frontend', start_new_session=True)
        try: stdout, stderr = run.communicate(json.dumps({'base':base,'token':token,'password':'isolated-reused-password-123','resume':resume}), timeout=60)
        except subprocess.TimeoutExpired:
            os.killpg(run.pid, signal.SIGTERM)
            try: run.communicate(timeout=5)
            except subprocess.TimeoutExpired: os.killpg(run.pid, signal.SIGKILL); run.communicate(timeout=5)
            raise AssertionError('Fixture browser timed out; own process group terminated')
        if run.returncode: print(stderr[-1500:])
        assert run.returncode == 0, 'Actual immutable ownership flow failed'
        return json.loads(stdout)
    with (directory/'server.log').open('w') as log:
        server = subprocess.Popen([str(BINARY)], env=env, cwd=directory, stdout=log, stderr=log)
        try:
            token = login(server); result = browser(token)
            path = directory/'auth/terminal_sessions.json'; metadata = json.loads(path.read_text())
            assert metadata[result['old']]['owner_id'] == result['old_owner']
            assert metadata[result['current']]['owner_id'] == result['new_owner']
            assert metadata[result['old']]['owner'] == metadata[result['current']]['owner'] == 'reused'
            for session in metadata.values():
                assert subprocess.run(['tmux','has-session','-t','='+session['tmux_name']],env=env,capture_output=True).returncode == 0
            print('PASS: deleted account and account-shared attachments close with 1008; same-name new ID cannot list/attach/mutate/share/read/kill old terminal, no local fallback, unrelated shell survives.')
            server.terminate(); server.wait(timeout=10)
            metadata = json.loads(path.read_text()); metadata[result['legacy']].pop('owner_id')
            path.write_text(json.dumps(metadata))
            server = subprocess.Popen([str(BINARY)], env=env, cwd=directory, stdout=log, stderr=log); token = login(server)
            assert browser(token, result) == {'resumed':True}
            persisted = json.loads(path.read_text())
            assert not persisted[result['legacy']].get('owner_id')
            assert persisted[result['current']]['owner_id'] == result['new_owner']
            assert subprocess.run(['tmux','has-session','-t','='+persisted[result['legacy']]['tmux_name']],env=env,capture_output=True).returncode == 0
            print('PASS: restart preserves current immutable ownership, denies deleted ID and missing-ID legacy without adopting by name, and leaves legacy shell/metadata intact.')
        finally:
            if server.poll() is None:
                server.terminate()
                try: server.wait(timeout=10)
                except subprocess.TimeoutExpired: server.kill(); server.wait(timeout=5)
            path = directory/'auth/terminal_sessions.json'
            if path.is_file():
                for session in json.loads(path.read_text()).values():
                    name = session.get('tmux_name','')
                    if name.startswith('nb_'): subprocess.run(['tmux','kill-session','-t','='+name],env=env,capture_output=True)
