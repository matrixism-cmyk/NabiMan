#!/usr/bin/env python3
"""Exercise account session management with a real isolated server and browser."""
import argparse, json, os, pathlib, signal, socket, subprocess, tempfile, time
from urllib.request import urlopen
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/debug/nabiman-server')).resolve()
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--browser',choices=['smoke-login-sessions.js','smoke-refresh-rotation.js'],default='smoke-login-sessions.js')
args=parser.parse_args()
with tempfile.TemporaryDirectory(prefix='nb-login-sessions-') as temporary:
    directory=pathlib.Path(temporary); (directory/'tmux').mkdir()
    with socket.socket() as sock: sock.bind(('127.0.0.1',0)); port=sock.getsockname()[1]
    password='isolated-session-fixture-password'
    env=dict(os.environ,NABIMAN_DATA_DIR=str(directory/'auth'),NABIMAN_MEC_DB=str(directory/'mec.db'),
        NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PORT=str(port),
        NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(directory/'tmux'))
    base=f'http://127.0.0.1:{port}'
    with (directory/'server.log').open('w') as log:
        server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log)
        try:
            for _ in range(600):
                assert server.poll() is None,'Fixture server exited'
                try:
                    with urlopen(base+'/',timeout=1) as response:
                        if response.status==200: break
                except OSError: pass
                time.sleep(.1)
            else: raise AssertionError('Fixture startup timeout')
            browser=subprocess.Popen(['node',str(ROOT/'frontend/scripts'/args.browser)],
                cwd=ROOT/'frontend',stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
            try: stdout,stderr=browser.communicate(json.dumps({'base':base,'password':password}),timeout=90)
            except subprocess.TimeoutExpired:
                os.killpg(browser.pid,signal.SIGTERM)
                try: browser.communicate(timeout=5)
                except subprocess.TimeoutExpired: os.killpg(browser.pid,signal.SIGKILL);browser.communicate(timeout=5)
                raise AssertionError('Browser fixture timeout')
            if browser.returncode: print(stderr[-1800:])
            assert browser.returncode==0,'Login session browser verification failed'
            print(stdout.strip())
        finally:
            server.terminate()
            try: server.wait(timeout=10)
            except subprocess.TimeoutExpired: server.kill();server.wait(timeout=5)

            registry=directory/'auth/terminal_sessions.json'
            if registry.is_file():
                for value in json.loads(registry.read_text()).values():
                    name=value.get('tmux_name','')
                    if name.startswith('nb_'): subprocess.run(['tmux','kill-session','-t','='+name],env=env,capture_output=True)
