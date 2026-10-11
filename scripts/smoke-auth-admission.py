#!/usr/bin/env python3
"""Isolated authentication admission: account/source/global limits, slow bodies and browser feedback."""
import http.client,json,os,pathlib,socket,subprocess,tempfile,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/release/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nb-auth-admission-') as temporary:
 d=pathlib.Path(temporary);(d/'tmux').mkdir()
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 base=f'http://127.0.0.1:{port}';password='isolated-admission-password'
 env={k:v for k,v in os.environ.items() if not k.startswith('NABIMAN_') and k not in ['TMUX','SSH_AUTH_SOCK','KUBECONFIG']}
 env.update(NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(d/'auth'),NABIMAN_MEC_DB=str(d/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(d/'tmux'))
 def call(path,body=None,peer='127.0.0.1',headers=None):
  c=http.client.HTTPConnection('127.0.0.1',port,timeout=10,source_address=(peer,0))
  try:
   c.request('GET' if body is None else 'POST',path,body=None if body is None else json.dumps(body),headers={'Content-Type':'application/json',**(headers or {})})
   r=c.getresponse();raw=r.read();return r.status,dict(r.getheaders()),json.loads(raw) if raw.startswith(b'{') else None
  finally:c.close()
 def start():
  p=subprocess.Popen([str(BINARY)],cwd=d,env=env,stdout=log,stderr=log)
  for _ in range(600):
   assert p.poll() is None,'Authentication fixture exited'
   try:
    if call('/')[0]==200:return p
   except OSError:pass
   time.sleep(.1)
  raise AssertionError('Fixture startup timeout')
 def stop(p):
  p.terminate()
  try:p.wait(timeout=10)
  except subprocess.TimeoutExpired:p.kill();p.wait()
 with (d/'server.log').open('w') as log:
  p=start()
  try:
   # 5 unknown-user failures and then a genuine account must remain independent.
   for i in range(5):
    assert call('/api/auth/login',{'username':'blocked','password':'wrong'},peer=f'127.0.0.{i+1}')[0]==401
   code,headers,_=call('/api/auth/2fa/verify',{'username':'blocked','password':'wrong','code':'000000'},headers={'X-Forwarded-For':'203.0.113.1'})
   assert code==429 and 1<=int(headers['retry-after'])<=300 and headers['cache-control']=='no-store'
   assert call('/api/auth/login',{'username':'admin','password':password})[0]==200
   result=subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-auth-admission.js')],input=json.dumps({'base':base,'password':password}),text=True,capture_output=True,timeout=50)
   assert result.returncode==0,'Authentication browser fixture failed; credential-bearing logs are not printed'
   browser=json.loads(result.stdout);assert browser['passed']
   # One stalled body is timed out before handlers; service remains usable.
   with socket.create_connection(('127.0.0.1',port),timeout=8) as sock:
    started=time.monotonic();sock.sendall(b'POST /api/auth/login HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 500\r\n\r\n{')
    response=sock.recv(1024);elapsed=time.monotonic()-started
    assert response.startswith(b'HTTP/1.1 400') and 2<=elapsed<7
   assert call('/')[0]==200
  finally:stop(p)
  # A fresh instance isolates aggregate counters from prior scenarios. Restart clears memory by design.
  p=start()
  try:
   for i in range(300):
    peer=f'127.0.0.{i//60+1}'
    assert call('/api/auth/login',{'username':f'unknown{i}','password':'wrong'},peer=peer)[0]==401,i
   code,headers,_=call('/api/auth/login',{'username':'another','password':'wrong'},peer='127.0.0.6')
   assert code==429 and 1<=int(headers['retry-after'])<=60
   assert call('/')[0]==200
  finally:stop(p)
 print(json.dumps({'passed':True,'cross_source_account_limit':True,'different_account_same_proxy_allowed':True,'forwarded_header_no_bypass':True,'retry_after_and_no_store':True,'slow_body_timeout':True,'global_limit_across_five_sources':True,'ordinary_requests_still_available':True,'browser':browser,'limitations':['single-process memory; restart resets counters','no production attacks or production account lockout']}))
