#!/usr/bin/env python3
"""Real isolated API: admin-only audit, retained corruption, sanitized export and persistence fault."""
import json,os,pathlib,socket,subprocess,tempfile,time
from urllib.request import Request,urlopen
from urllib.error import HTTPError
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/debug/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nb-audit-') as tmp:
 d=pathlib.Path(tmp);(d/'tmux').mkdir();(d/'auth').mkdir(mode=0o700)
 original=b'historical malformed line\n';audit=d/'auth/audit.jsonl';audit.write_bytes(original);audit.chmod(0o600)
 rotated=d/'auth/audit.jsonl.1';rotated.write_text(json.dumps({'timestamp':'fixture','user':'admin','method':'GET','path':'/api/share/SECRET_CAP/auth?ticket=SECRET_QUERY','status':200,'ip':'127.0.0.1','hash':None})+'\n');rotated.chmod(0o600)
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 base=f'http://127.0.0.1:{port}';password='isolated-audit-fixture-password'
 env={k:v for k,v in os.environ.items() if not k.startswith('NABIMAN_') and k not in ['TMUX','SSH_AUTH_SOCK','KUBECONFIG']};env.update(NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(d/'auth'),NABIMAN_MEC_DB=str(d/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(d/'tmux'))
 def call(path,body=None,token=None,method=None):
  headers={'Content-Type':'application/json'}
  if token:headers['Authorization']='Bearer '+token
  try:r=urlopen(Request(base+path,data=None if body is None else json.dumps(body).encode(),headers=headers,method=method),timeout=30)
  except HTTPError as e:r=e
  with r:
   payload=r.read()
   try:value=json.loads(payload)
   except ValueError:value=None
   return r.status,value,{k.lower():v for k,v in r.headers.items()}
 def login(username='admin'):
  code,data,_=call('/api/auth/login',{'username':username,'password':password});assert code==200 and data['success'];return data['data']['token']
 def start():
  p=subprocess.Popen([str(BINARY)],cwd=d,env=env,stdout=log,stderr=log)
  for _ in range(600):
   assert p.poll() is None,'Audit fixture exited'
   try:
    with urlopen(base+'/',timeout=1) as r:
     if r.status==200:return p
   except OSError:pass
   time.sleep(.1)
  p.terminate();p.wait();raise AssertionError('Startup timeout')
 def stop(p):
  p.terminate()
  try:p.wait(timeout=10)
  except subprocess.TimeoutExpired:p.kill();p.wait()
 with (d/'server.log').open('w') as log:
  p=start()
  try:
   token=login();health=call('/api/audit/health',token=token)[1]['data'];assert health['writable'] and health['unparsed_startup_lines']==1
   for role in ['viewer','operator']:
    assert call('/api/users',{'username':role,'password':password,'role':role},token)[1]['success'];member=login(role)
    for endpoint,body in [('/api/audit',None),('/api/audit/health',None),('/api/audit/verify',None),('/api/audit/forwarding',None),('/api/audit/export',{'segment':'audit.jsonl'}),('/api/audit/clear',{})]:assert call(endpoint,body,member)[0]==403
   for endpoint in ['/api/%61udit','/api/audit%2Fverify']:
    assert call(endpoint,token=member)[0] in [403,404], 'Encoded audit path bypassed authorization'
   verification=call('/api/audit/verify',token=token)[1]['data'];assert not verification['valid'] and len(verification['segments'])==2
   assert next(s for s in verification['segments'] if s['name']=='audit.jsonl')['malformed_lines']==1
   code,bundle,headers=call('/api/audit/export',{'segment':'audit.jsonl.1'},token);assert code==200 and headers.get('cache-control')=='no-store'
   text=json.dumps(bundle);assert 'SECRET_CAP' not in text and 'SECRET_QUERY' not in text and bundle['data']['sanitized']
   assert call('/api/audit/export',{'segment':'../users.json'},token)[0]==400
   assert call('/api/audit/clear',{},token)[0]==410 and audit.read_bytes().startswith(original)
   if os.environ.get('NABIMAN_AUDIT_BROWSER'):
    result=subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-audit.js')],input=json.dumps({'base':base,'token':token}),text=True,capture_output=True,timeout=100);assert result.returncode==0,result.stderr[-1000:]
   saved=d/'saved.jsonl';audit.rename(saved);audit.mkdir()
   code,data,headers=call('/api/users/me',token=token);assert code==200 and headers.get('x-nabiman-audit')=='unavailable'
   state=call('/api/audit/health',token=token)[1]['data'];assert not state['writable'] and state['error']
   audit.rmdir();saved.rename(audit);assert not call('/api/audit/health',token=token)[1]['data']['writable']
   stop(p);p=start();token=login();assert call('/api/audit/health',token=token)[1]['data']['writable']
   assert audit.read_bytes().startswith(original)
   print('PASS: audit admin-only access, retained malformed evidence, per-file verification, sanitized export, clear refusal, disk failure signal without false operation rollback, explicit restart recovery'+(', browser review/download flow' if os.environ.get('NABIMAN_AUDIT_BROWSER') else '')+'.')
  finally:stop(p)
