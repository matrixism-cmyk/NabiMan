#!/usr/bin/env python3
"""Actual runtime token admission, durable revocation, owner binding and legacy preservation."""
import http.client,json,os,pathlib,socket,subprocess,tempfile,time
from urllib.request import Request,urlopen
from urllib.error import HTTPError
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/release/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nb-tokens-') as temporary:
 d=pathlib.Path(temporary);(d/'auth').mkdir();(d/'tmux').mkdir()
 legacy={'id':'legacy-fixture','name':'Old key','key_hash':'a'*64,'prefix':'nbm_old','role':'admin','created_at':'2001-01-01'}
 (d/'auth/api_keys.json').write_text(json.dumps([legacy]))
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 base=f'http://127.0.0.1:{port}';password='isolated-token-password'
 env={k:v for k,v in os.environ.items() if not k.startswith('NABIMAN_') and k not in ['TMUX','SSH_AUTH_SOCK','KUBECONFIG']}
 env.update(NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(d/'auth'),NABIMAN_MEC_DB=str(d/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(d/'tmux'))
 def call(path,body=None,token=None,method=None,extra=None):
  h={'Content-Type':'application/json'}
  if token:h['Authorization']='Bearer '+token
  if extra:h.update(extra)
  try:r=urlopen(Request(base+path,data=None if body is None else json.dumps(body).encode(),headers=h,method=method),timeout=20)
  except HTTPError as e:r=e
  with r:
   raw=r.read()
   try:value=json.loads(raw)
   except ValueError:value=None
   return r.status,value,dict(r.headers)
 def login(name='admin'):
  code,value,_=call('/api/auth/login',{'username':name,'password':password});assert code==200;return value['data']['token']
 def start():
  p=subprocess.Popen([str(BINARY)],cwd=d,env=env,stdout=log,stderr=log)
  for _ in range(600):
   assert p.poll() is None,'Token fixture exited'
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
 def create(scopes=['status:read'],days=1):
  code,value,headers=call('/api/apikeys',{'name':'Fixture token','scopes':scopes,'expires_in_days':days},jwt)
  assert code==200 and value['success'] and headers.get('cache-control',headers.get('Cache-Control'))=='no-store'
  return value['data']
 with (d/'server.log').open('w') as log:
  p=start()
  try:
   jwt=login();assert call('/api/apikeys',token=jwt)[1]['data'][0]['status']=='legacy_disabled'
   for body in [{'name':'bad','scopes':['status:read'],'expires_in_days':v} for v in [0,91,2**64]]+[{'name':'bad','role':'admin'},{'name':'bad','scopes':['*'],'expires_in_days':1}]:assert call('/api/apikeys',body,jwt)[0]==400
   assert call('/api/users',{'username':'viewer','password':password,'role':'viewer'},jwt)[1]['success'];viewer=login('viewer')
   assert call('/api/apikeys',token=viewer)[0]==403
   token=create();raw=token['key'];assert len(raw)==68
   assert call('/api/server/status',token=raw)[0]==200
   assert call('/api/server/status',extra={'X-API-Key':raw})[0]==200
   for path in ['/api/users','/api/apikeys','/api/terminal','/api/remote-servers','/api/audit/health','/api/mec/v1/tenants','/api/server/status/extra','/api/%73erver/status']:
    assert call(path,token=raw)[0]==403,path
   assert call('/api/apikeys',{'name':'escalation'},raw)[0]==403
   assert call('/api/server/status?token='+raw)[0]==401
   assert call('/api/server/status',token=jwt,extra={'X-API-Key':raw})[0]==401
   assert call('/api/server/status',extra={'X-API-Key':raw,'X-Auth-Token':jwt})[0]==401
   conn=http.client.HTTPConnection('127.0.0.1',port);conn.putrequest('GET','/api/server/status');conn.putheader('X-API-Key',raw);conn.putheader('X-API-Key',raw);conn.endheaders();r=conn.getresponse();assert r.status==401;r.read();conn.close()
   listing=call('/api/apikeys',token=jwt)[1]['data'];assert raw not in json.dumps(listing) and 'key_hash' not in json.dumps(listing)
   assert next(k for k in listing if k['id']==token['id'])['last_used']
   path=d/'auth/api_keys.json';assert raw not in path.read_text();assert path.stat().st_mode&0o777==0o600
   stop(p);p=start();jwt=login();assert call('/api/server/status',token=raw)[0]==200
   stop(p);env['NABIMAN_API_TOKENS_DISABLED']='1';p=start();jwt=login();assert call('/api/server/status',token=raw)[0]==401
   assert next(k for k in call('/api/apikeys',token=jwt)[1]['data'] if k['id']==token['id'])['status']=='operator_disabled'
   stop(p);env.pop('NABIMAN_API_TOKENS_DISABLED');p=start();jwt=login();assert call('/api/server/status',token=raw)[0]==200
   inventory=create(['servers:read']);assert call('/api/remote-servers',token=inventory['key'])[0]==200;assert call('/api/server/status',token=inventory['key'])[0]==403
   tenants=create(['tenants:read']);assert call('/api/mec/v1/tenants',token=tenants['key'])[0]==200
   assert call('/api/apikeys/'+token['id'],token=jwt,method='DELETE')[0]==200;assert call('/api/server/status',token=raw)[0]==401
   stop(p);p=start();jwt=login();assert call('/api/server/status',token=raw)[0]==401
   changed=create();stop(p)
   users=json.loads((d/'auth/users.json').read_text());users[0]['id']='different-immutable-owner';(d/'auth/users.json').write_text(json.dumps(users));p=start();jwt=login()
   assert call('/api/server/status',token=changed['key'])[0]==401
   assert next(k for k in call('/api/apikeys',token=jwt)[1]['data'] if k['id']==changed['id'])['status']=='owner_changed'
   if os.environ.get('NABIMAN_TOKENS_BROWSER'):
    result=subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-api-tokens.js')],input=json.dumps({'base':base,'token':jwt}),text=True,capture_output=True,timeout=90);assert result.returncode==0,result.stderr[-1500:]
   # A successful token call records the token ID and owner, never the bearer secret.
   audit=(d/'auth/audit.jsonl').read_text();assert 'api-token:'+token['id']+';owner:' in audit and raw not in audit
   assert json.loads(path.read_text())[0]['key_hash']==legacy['key_hash']
   print('PASS: real Bearer/X-API-Key admission; exact read scopes/default deny; mixed/duplicate/URL credential refusal; bounded expiry; legacy disabled; 0600 hash-only persistence; restart and durable revocation; immutable-owner invalidation; token-specific audit'+('; browser one-time issue/revoke' if os.environ.get('NABIMAN_TOKENS_BROWSER') else ''))
  finally:stop(p)
