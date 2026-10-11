#!/usr/bin/env python3
"""Real MFA enrollment/recovery across process restart; secrets remain in private fixture memory."""
import argparse, json, os, pathlib, signal, socket, sqlite3, subprocess, tempfile, time
from urllib.request import urlopen
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/debug/nabiman-server')).resolve()
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--policy',action='store_true')
parser.add_argument('--history',action='store_true')
args=parser.parse_args()
with tempfile.TemporaryDirectory(prefix='nb-mfa-') as temporary:
    directory=pathlib.Path(temporary);(directory/'tmux').mkdir()
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    password='isolated-mfa-fixture-password'
    env=dict(os.environ,NABIMAN_DATA_DIR=str(directory/'auth'),NABIMAN_MEC_DB=str(directory/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PORT=str(port),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(directory/'tmux'))
    base=f'http://127.0.0.1:{port}'
    if args.policy: env['NABIMAN_CORS_ORIGINS']=f'http://localhost:{port},{base}'
    def stop(server):
        if server and server.poll() is None:
            server.terminate()
            try:server.wait(timeout=10)
            except subprocess.TimeoutExpired:server.kill();server.wait(timeout=5)
    def start(log):
        process=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log)
        try:
            for _ in range(600):
                assert process.poll() is None,'MFA fixture exited'
                try:
                    with urlopen(base+'/',timeout=1) as response:
                        if response.status==200:return process
                except OSError:pass
                time.sleep(.1)
            raise AssertionError('MFA fixture startup timeout')
        except BaseException:stop(process);raise
    def browser(resume=None):
        process=subprocess.Popen(['node',str(ROOT/'frontend/scripts'/('smoke-policy-history.js' if args.history else 'smoke-mfa-policy.js' if args.policy else 'smoke-mfa.js'))],cwd=ROOT/'frontend',stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
        try:stdout,stderr=process.communicate(json.dumps({'base':base,'password':password,'authFile':str(directory/'auth/users.json'),'resume':resume,'policyFile':str(directory/'auth/scope-settings.json')}),timeout=100)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGTERM)
            try:process.communicate(timeout=5)
            except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.communicate(timeout=5)
            raise AssertionError('MFA browser timeout')
        if process.returncode:print(stderr[-1800:])
        assert process.returncode==0,'MFA browser verification failed'
        return json.loads(stdout)
    server=None
    with (directory/'server.log').open('w') as log:
        try:
            server=start(log)
            if args.policy:
                with sqlite3.connect(directory/'mec.db') as db:
                    db.execute('INSERT INTO tenant_profiles (id,display_name,namespace,allocation_json,quota_json,status,created_at,created_by,updated_at) VALUES (?,?,?,?,?,?,?,?,?)',
                        ('mfa-tenant','MFA tenant','mfa-tenant',json.dumps({'type':'shared'}),json.dumps({'cpu_requests':'1','cpu_limits':'2','memory_requests':'1Gi','memory_limits':'2Gi','gpu':0,'storage':'10Gi','pods':10,'pvcs':2,'lb_services':1}),json.dumps({'state':'active'}),'2026-10-06T00:00:00Z','admin','2026-10-06T00:00:00Z'))
            resume=browser();stop(server)
            server=start(log);assert browser(resume)=={'passed':True}
            print('PASS: policy revisions, stale/concurrent writes, manager-only recent history, browser conflict recovery and restart persistence' if args.history else 'PASS: MFA scope policy enforcement, browser enrollment gate, multiple scopes, manager restrictions, restart persistence and fail-closed configuration' if args.policy else 'PASS: scoped member QR enrollment and one-use recovery; secret API omission and hashed storage; session invalidation; consumed codes remain denied after real restart; regeneration and authenticated disable through actual browser.')
        finally:stop(server)
