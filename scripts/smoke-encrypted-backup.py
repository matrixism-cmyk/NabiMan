#!/usr/bin/env python3
"""Encrypted archive/UI lifecycle in private auth/DB/files/tmux; never touch production data."""
import json,os,pathlib,socket,subprocess,tempfile,time,signal
from urllib.request import Request,urlopen
from urllib.error import HTTPError
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/debug/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nb-encrypted-') as tmp:
 d=pathlib.Path(tmp);(d/'tmux').mkdir();(d/'source').mkdir();(d/'source/probe.txt').write_text('encrypted fixture contents\n')
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 base=f'http://127.0.0.1:{port}';password='isolated-backup-fixture-password'
 env=dict(os.environ,NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(d/'auth'),NABIMAN_MEC_DB=str(d/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(d/'tmux'))
 def call(path,body=None,token=None,method=None):
  headers={'Content-Type':'application/json'}
  if token:headers['Authorization']='Bearer '+token
  try:r=urlopen(Request(base+path,data=None if body is None else json.dumps(body).encode(),headers=headers,method=method),timeout=120 if path=='/api/backup/restore-encrypted' else 10)
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
   process=subprocess.Popen(['node',str(ROOT/'frontend/scripts/smoke-encrypted-backup.js')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
   try:stdout,stderr=process.communicate(json.dumps({'base':base,'password':password,'source':str(d/'source'),'directory':str(d)}),timeout=180)
   except subprocess.TimeoutExpired:
    os.killpg(process.pid,signal.SIGTERM)
    try:process.communicate(timeout=5)
    except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.communicate(timeout=5)
    raise AssertionError('Encrypted backup browser timeout')
   assert process.returncode==0,stderr[-1600:]
   proof=json.loads(stdout)
   artifact=d/'auth/backups'/proof['filename']
   assert artifact.read_bytes().startswith(b'age-encryption.org/v1\n')
   assert not any(p.name.startswith('.') for p in (d/'auth/backups').iterdir())
   assert (d/'restored'/str(d/'source/probe.txt').lstrip('/')).read_text()=='encrypted fixture contents\n'
   reference=os.environ.get('NABIMAN_AGE_REFERENCE')
   if reference:
    def interop(mode,file):
     r=subprocess.run([reference],input=json.dumps({'Mode':mode,'File':str(file),'Password':'fixture encrypted passphrase long'}),text=True,capture_output=True,timeout=30)
     assert r.returncode==0,'Reference age interoperability failed'
    interop('decrypt',artifact)
    imported=d/'auth/backups/reference.tar.gz.age';interop('encrypt',imported)
    token=call('/api/auth/login',{'username':'admin','password':password})[1]['data']['token']
    response=call('/api/backup/restore-encrypted',{'filename':imported.name,'target_dir':str(d/'reference-restored'),'password':'fixture encrypted passphrase long'},token)
    assert response[0]==200 and response[1]['success']
    assert (d/'reference-restored/reference.txt').read_text()=='reference age fixture\n'
   print('PASS: age browser create/restore, wrong-password/tamper/truncation/overwrite rejection, password clearing, private artifact, no plaintext leftovers'+(', independent Go age bidirectional interoperability' if reference else '')+'.')

  finally:stop(p)
