#!/usr/bin/env python3
"""Real archive/UI lifecycle in private auth/DB/files/tmux; never touch production schedules."""
import json,os,pathlib,socket,subprocess,tempfile,time,tarfile
from urllib.request import Request,urlopen
from urllib.error import HTTPError
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/debug/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nb-backup-') as tmp:
 d=pathlib.Path(tmp);(d/'tmux').mkdir();(d/'source').mkdir();(d/'source/probe.txt').write_text('backup restore fixture\n')
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 base=f'http://127.0.0.1:{port}';password='isolated-backup-fixture-password'
 env=dict(os.environ,NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(d/'auth'),NABIMAN_MEC_DB=str(d/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(d/'tmux'))
 def call(path,body=None,token=None,method=None):
  headers={'Content-Type':'application/json'}
  if token:headers['Authorization']='Bearer '+token
  try:r=urlopen(Request(base+path,data=None if body is None else json.dumps(body).encode(),headers=headers,method=method),timeout=10)
  except HTTPError as e:r=e
  with r:return r.status,json.load(r)
 def start():
  p=subprocess.Popen([str(BINARY)],cwd=d,env=env,stdout=log,stderr=log)
  for _ in range(600):
   assert p.poll() is None,'fixture exited'
   try:
    with urlopen(base+'/',timeout=1) as r:
     if r.status==200:return p
   except OSError:pass
   time.sleep(.1)
  p.terminate();p.wait();raise AssertionError('startup timeout')
 def stop(p):
  p.terminate()
  try:p.wait(timeout=10)
  except subprocess.TimeoutExpired:p.kill();p.wait()
 with (d/'server.log').open('w') as log:
  p=start()
  try:
   result=subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-backup-schedules.js')],input=json.dumps({'base':base,'password':password,'source':str(d/'source')}),text=True,capture_output=True,timeout=100)
   assert result.returncode==0,result.stderr[-1600:]
   proof=json.loads(result.stdout);artifact=d/'auth/backups'/proof['filename'];assert artifact.stat().st_mode&0o777==0o600
   with tarfile.open(artifact) as archive:
    target=d/'restored';target.mkdir();archive.extractall(target,filter='data');assert (target/str(d/'source/probe.txt').lstrip('/')).read_text()=='backup restore fixture\n'
   stop(p)
   path=d/'auth/backup_schedules.json';rows=json.loads(path.read_text());row=next(r for r in rows if r['id']==proof['id']);row['last_result']['state']='running';row['last_result']['finished_at']=None;row['last_result']['filename']=None;path.write_text(json.dumps(rows))
   p=start();token=call('/api/auth/login',{'username':'admin','password':password})[1]['data']['token'];saved=call('/api/backup/schedules',token=token)[1]['data'];assert next(r for r in saved if r['id']==proof['id'])['last_result']['state']=='interrupted'
   stop(p);path.write_text('CORRUPT_FIXTURE_DO_NOT_REPLACE');p=start();token=call('/api/auth/login',{'username':'admin','password':password})[1]['data']['token'];assert call('/api/backup/schedules',token=token)[0]==503;assert call('/api/backup/schedules',{'name':'test','paths':[str(d/'source')],'cron_expr':'0 0 * * *'},token)[0]==503;assert path.read_text()=='CORRUPT_FIXTURE_DO_NOT_REPLACE'
   print('PASS: schedule UI creation, real archive and extracted byte equality, explicit failure, permissions, non-admin denial, restart interruption and corrupt-store preservation.')
  finally:stop(p)
