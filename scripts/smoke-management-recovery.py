#!/usr/bin/env python3
"""Full isolated stopped-server capture -> age restore -> real login and scope verification."""
import json,os,pathlib,socket,sqlite3,subprocess,tempfile,time
from urllib.request import Request,urlopen
from urllib.error import HTTPError
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/release/nabiman-server')).resolve()
TOOL=pathlib.Path(os.environ.get('NABIMAN_RECOVERY_TOOL',BINARY.parent/'nabiman-recovery')).resolve()
CAPTURE_TOOL=pathlib.Path(os.environ.get('NABIMAN_RECOVERY_CAPTURE_TOOL',TOOL)).resolve()
SCRIPT=ROOT/'scripts/management-recovery.py'
with tempfile.TemporaryDirectory(prefix='nb-recovery-') as tmp:
 d=pathlib.Path(tmp);(d/'tmux').mkdir();password='fixture-account-password';bundle_password='fixture-recovery-passphrase-unique'
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 base=f'http://127.0.0.1:{port}'
 env={k:v for k,v in os.environ.items() if not k.startswith('NABIMAN_') and k not in ['TMUX','SSH_AUTH_SOCK','KUBECONFIG']};env.update(NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(d/'auth'),NABIMAN_MEC_DB=str(d/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(d/'tmux'))
 def api(path,body=None,token=None,method=None):
  h={'Content-Type':'application/json'}
  if token:h['Authorization']='Bearer '+token
  try:r=urlopen(Request(base+path,data=None if body is None else json.dumps(body).encode(),headers=h,method=method),timeout=15)
  except HTTPError as e:r=e
  with r:return r.status,json.load(r)
 def login(name='admin'):
  code,result=api('/api/auth/login',{'username':name,'password':password});assert code==200 and result['success'];return result['data']['token']
 def start(binary):
  p=subprocess.Popen([str(binary)],cwd=d,env=env,stdout=log,stderr=log)
  for _ in range(600):
   assert p.poll() is None,'Fixture server exited'
   try:
    with urlopen(base+'/',timeout=1) as r:
     if r.status==200:return p
   except OSError:pass
   time.sleep(.1)
  p.terminate();p.wait();raise AssertionError('Fixture startup timeout')
 def stop(p):
  p.terminate()
  try:p.wait(timeout=10)
  except subprocess.TimeoutExpired:p.kill();p.wait()
 def recover(arguments,success=True):
  result=subprocess.run(['python3',str(SCRIPT),'--tool',str(CAPTURE_TOOL if arguments[0]=='capture' else TOOL),'--passphrase-fd','0',*map(str,arguments)],input=bundle_password+'\n',text=True,capture_output=True,timeout=600)
  if success:
   assert result.returncode==0,result.stderr[-600:];return json.loads(result.stdout)
  assert result.returncode!=0,'Unsafe recovery accepted';return result.stderr
 with (d/'server.log').open('w') as log:
  p=start(BINARY)
  try:
   token=login();admin=api('/api/users/me',token=token)[1]['data']
   code,server=api('/api/remote-servers',{'name':'Recovery fixture','host':'recovery.invalid','user':'fixture','port':22,'auth_method':'password','password':'FIXTURE_SERVER_SECRET'},token);assert code==200 and server['success'];sid=server['data']['id']
   code,result=api('/api/users',{'username':'member','password':password,'role':'viewer'},token);assert result['success']
   users=api('/api/users',token=token)[1]['data'];member=next(u for u in users if u['username']=='member')
   assert api('/api/users/'+member['id']+'/review',{'status':'active','memberships':[{'kind':'server','scope_id':sid,'role':'operator'}]},token)[1]['success']
   endpoint='/api/scope-settings/server/'+sid;policy=api(endpoint,token=token)[1]['data'];policy.update(change_reason='Recovery fixture policy',expected_revision=policy.pop('revision'),member_limit=10)
   assert api(endpoint,policy,token,'PUT')[1]['success']
   original_policy=api(endpoint,token=token)[1]['data'];original_history=api(endpoint+'/history',token=token)[1]['data'];original_member=api('/api/users/me',token=login('member'))[1]['data']
   command=['capture','--auth',d/'auth','--database',d/'mec.db','--runtime',BINARY,'--static',ROOT/'frontend/build','--output',d/'archives']
   assert 'still owns' in recover(command,False)
   with sqlite3.connect(d/'mec.db') as db:db.execute("INSERT INTO mec_config(key,value,updated_at) VALUES('recovery-fixture','preserved','2026-10-06')")
   stop(p)
   proof=recover(command);assert proof['verification']['accounts']==2 and proof['verification']['encrypted_credentials_verified']==1
   archive=pathlib.Path(proof['archive']);assert archive.stat().st_mode&0o777==0o600
   target=d/'restored';restored=recover(['restore','--archive',archive,'--target',target]);assert restored['snapshot_id']==proof['snapshot_id'];assert restored['runtime_sha256']==proof['verification']['runtime_sha256']
   assert recover(['restore','--archive',archive,'--target',target],False)
   with sqlite3.connect(target/'database/mec.db') as db:assert db.execute("SELECT value FROM mec_config WHERE key='recovery-fixture'").fetchone()[0]=='preserved'
   for name in ['users.json','jwt_secret','secret.key','remote_servers.json','scope-settings.json','terminal_sessions.json','terminal_settings.json','terminal_shares.json']:
    assert (d/'auth'/name).read_bytes()==(target/'auth'/name).read_bytes(),name+' changed in restore'
   assert not (target/'auth/terminal_sessions.lock').exists()
   env.update(NABIMAN_DATA_DIR=str(target/'auth'),NABIMAN_MEC_DB=str(target/'database/mec.db'),NABIMAN_STATIC=str(target/'runtime/static'))
   p=start(target/'runtime/nabiman-server');token=login();identity=api('/api/users/me',token=token)[1]['data'];assert identity['id']==admin['id']
   restored_member=api('/api/users/me',token=login('member'))[1]['data'];assert restored_member['id']==original_member['id'] and restored_member['memberships']==original_member['memberships']
   assert api(endpoint,token=token)[1]['data']==original_policy;assert api(endpoint+'/history',token=token)[1]['data']==original_history
   assert api('/api/diagnostics/runtime',token=token)[1]['data']['executable_sha256']==restored['runtime_sha256']
   assert api('/api/remote-servers',token=token)[1]['data'][0]['has_password']
   print('PASS: live ownership refused; stopped management snapshot, encrypted credential/key verification, portable age restore, overwrite refusal, DB value and policy history preserved; recovered runtime actual admin/member login, immutable identity, memberships and runtime digest verified.')
  finally:stop(p)
