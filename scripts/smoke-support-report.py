#!/usr/bin/env python3
"""Verify a real support report without exporting fixture credentials or raw logs."""
import hashlib,json,os,pathlib,signal,socket,sqlite3,subprocess,tempfile,time
from urllib.request import urlopen
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/debug/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nb-support-') as temporary:
    directory=pathlib.Path(temporary);(directory/'tmux').mkdir()
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    password='CANARY_PRIVATE_PASSWORD_123'
    env=dict(os.environ,NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(directory/'auth'),NABIMAN_MEC_DB=str(directory/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(directory/'tmux'),NABIMAN_SUPPORT_CANARY='CANARY_ENVIRONMENT_SECRET')
    base=f'http://127.0.0.1:{port}'
    with (directory/'server.log').open('w') as log:
        server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log)
        try:
            for _ in range(600):
                assert server.poll() is None,'Support fixture exited'
                try:
                    with urlopen(base+'/',timeout=1) as response:
                        if response.status==200:break
                except OSError:pass
                time.sleep(.1)
            else:raise AssertionError('Support fixture startup timeout')
            with sqlite3.connect(directory/'mec.db') as db:
                db.execute('INSERT INTO jobs (id,kind,status,steps,created_by,started_at) VALUES (?,?,?,?,?,?)',('CANARY_JOB_ID',json.dumps({'kind':'doc_generate','template':'CANARY_TEMPLATE','tenant_ids':[]}),json.dumps({'state':'failed','error':'CANARY_JOB_PASSWORD'}),'[]','CANARY_JOB_ACTOR','2026-10-06T00:00:00Z'))
            process=subprocess.Popen(['node',str(ROOT/'frontend/scripts/smoke-support-report.js')],cwd=ROOT/'frontend',stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
            try:stdout,stderr=process.communicate(json.dumps({'base':base,'password':password,'authDirectory':str(directory/'auth'),'binarySha256':hashlib.sha256(BINARY.read_bytes()).hexdigest()}),timeout=100)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid,signal.SIGTERM)
                try:process.communicate(timeout=5)
                except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.communicate(timeout=5)
                raise AssertionError('Support browser timeout')
            if process.returncode:print(stderr[-1800:])
            assert process.returncode==0,'Support report verification failed'
            print(stdout.strip())
        finally:
            server.terminate()
            try:server.wait(timeout=10)
            except subprocess.TimeoutExpired:server.kill();server.wait(timeout=5)
