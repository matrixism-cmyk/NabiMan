#!/usr/bin/env python3
"""Isolated HTTPS receiver: certificate checks, durable retries, redacted credentials and pause."""
import http.server,json,os,pathlib,socket,sqlite3,ssl,subprocess,tempfile,threading,time
from urllib.request import Request,urlopen
from urllib.error import HTTPError
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/debug/nabiman-server')).resolve()
with tempfile.TemporaryDirectory(prefix='nb-forward-') as temporary:
 d=pathlib.Path(temporary);(d/'tmux').mkdir();(d/'auth').mkdir(mode=0o700)
 cert=d/'cert.pem';key=d/'key.pem'
 subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(key),'-out',str(cert),'-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost','-addext','basicConstraints=critical,CA:FALSE','-addext','extendedKeyUsage=serverAuth'],check=True,capture_output=True)
 received=[];mode={'status':503};lock=threading.Lock()
 class Receiver(http.server.BaseHTTPRequestHandler):
  def do_POST(self):
   payload=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
   with lock:received.append({'path':self.path,'id':self.headers.get('Idempotency-Key'),'auth':self.headers.get('Authorization'),'payload':payload})
   status=302 if self.path=='/redirect' else mode['status'];self.send_response(status)
   if status==302:self.send_header('Location','/redirected')
   self.end_headers()
   try:self.wfile.write(b'fixture receiver response must not be reflected')
   except OSError:pass
  def log_message(self,*args):pass
 receiver=http.server.ThreadingHTTPServer(('127.0.0.1',0),Receiver)
 context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(cert,key);receiver.socket=context.wrap_socket(receiver.socket,server_side=True)
 thread=threading.Thread(target=receiver.serve_forever,daemon=True);thread.start();receiver_url=f'https://localhost:{receiver.server_port}'
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 base=f'http://127.0.0.1:{port}';password='isolated-forward-password';secret='Bearer PRIVATE_FORWARD_FIXTURE'
 env={k:v for k,v in os.environ.items() if not k.startswith('NABIMAN_') and k not in ['TMUX','SSH_AUTH_SOCK','KUBECONFIG']};env.update(NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(d/'auth'),NABIMAN_MEC_DB=str(d/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(d/'tmux'),NABIMAN_AUDIT_CA_FILE=str(cert))
 def call(path,body=None,token=None):
  headers={'Content-Type':'application/json'}
  if token:headers['Authorization']='Bearer '+token
  try:r=urlopen(Request(base+path,data=None if body is None else json.dumps(body).encode(),headers=headers),timeout=30)
  except HTTPError as error:r=error
  with r:return r.status,json.load(r)
 def login(username='admin'):
  code,result=call('/api/auth/login',{'username':username,'password':password});assert code==200;return result['data']['token']
 def start():
  p=subprocess.Popen([str(BINARY)],cwd=d,env=env,stdout=log,stderr=log)
  for _ in range(600):
   assert p.poll() is None,'Forwarding fixture exited'
   try:
    with urlopen(base+'/',timeout=1) as response:
     if response.status==200:return p
   except OSError:pass
   time.sleep(.1)
  p.terminate();p.wait();raise AssertionError('Fixture startup timeout')
 def stop(p):
  p.terminate()
  try:p.wait(timeout=10)
  except subprocess.TimeoutExpired:p.kill();p.wait()
 def status():
  code,data=call('/api/audit/forwarding',token=token);assert code==200;assert secret not in json.dumps(data);return data['data']
 def dest(key):return next(row for row in status()['destinations'] if row['id']==key)
 def change(key,action):
  return call(f'/api/audit/forwarding/{key}/{action}',{'revision':dest(key)['revision']},token)
 def wait(predicate,message):
  for _ in range(300):
   if predicate():return
   time.sleep(.1)
  raise AssertionError(message+'; state='+json.dumps(status())+'; received_count='+str(len(received)))
 with (d/'server.log').open('w') as log:
  p=start()
  try:
   token=login()
   assert call('/api/audit/forwarding',{'name':'unsafe','url':'http://localhost:1','auth_header':secret},token)[0]==400
   code,result=call('/api/audit/forwarding',{'name':'Receiver','url':receiver_url+'/events','auth_header':secret},token);assert code==200;target=result['data']['id'];assert not dest(target)['enabled'] and not received
   assert call('/api/users',{'username':'viewer','password':password,'role':'viewer'},token)[1]['success'];viewer=login('viewer')
   for endpoint,body in [('/api/audit/forwarding',None),('/api/audit/forwarding',{'name':'denied','url':receiver_url,'auth_header':secret}),(f'/api/audit/forwarding/{target}/test',{'revision':dest(target)['revision']})]:assert call(endpoint,body,viewer)[0]==403
   assert change(target,'test')[0]==202
   wait(lambda:dest(target)['last_error']=='http_rejected_503','Receiver rejection was reported as success')
   assert dest(target)['acknowledged']==0
   with lock:first=received[0];assert first['auth']==secret;stable_id=first['id']
   db_path=d/'auth/audit-forwarding.sqlite3';assert db_path.stat().st_mode&0o777==0o600;assert secret.encode() not in db_path.read_bytes()
   stop(p);env['NABIMAN_AUDIT_FORWARDING_DISABLED']='1';p=start();token=login();assert status()['delivery_paused_by_operator'];assert dest(target)['pending']==1
   with lock:before=len(received)
   time.sleep(1)
   with lock:assert len(received)==before
   stop(p);env.pop('NABIMAN_AUDIT_FORWARDING_DISABLED');mode['status']=204
   with sqlite3.connect(db_path) as db:db.execute('UPDATE deliveries SET next_attempt=0')
   p=start();token=login();wait(lambda:dest(target)['acknowledged']==1,'Persisted delivery did not resume')
   with lock:assert len([row for row in received if row['id']==stable_id])>=2
   assert change(target,'enable')[0]==200;assert call('/api/users/me',token=token)[0]==200
   wait(lambda:dest(target)['acknowledged']>=2,'Automatic audit forwarding did not run')
   assert change(target,'disable')[0]==200
   # A trusted CA is insufficient when the certificate hostname does not match.
   code,result=call('/api/audit/forwarding',{'name':'Wrong hostname','url':receiver_url.replace('localhost','127.0.0.1')+'/wrong','auth_header':secret},token);bad=result['data']['id'];assert change(bad,'test')[0]==202
   wait(lambda:dest(bad)['last_error']=='tls_or_transport_failed','Hostname verification not enforced')
   with lock:assert not any(row['path']=='/wrong' for row in received)
   assert change(bad,'delete')[0]==200
   code,result=call('/api/audit/forwarding',{'name':'Redirect','url':receiver_url+'/redirect','auth_header':secret},token);redirect=result['data']['id'];assert change(redirect,'test')[0]==202
   wait(lambda:dest(redirect)['last_error']=='http_rejected_302','Redirect accepted')
   with lock:assert not any(row['path']=='/redirected' for row in received)
   assert change(redirect,'delete')[0]==200
   if os.environ.get('NABIMAN_FORWARD_BROWSER'):
    result=subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-audit-forwarding.js')],input=json.dumps({'base':base,'token':token,'receiver':receiver_url+'/browser'}),text=True,capture_output=True,timeout=100);assert result.returncode==0,result.stderr[-1000:]
   stop(p);env.pop('NABIMAN_AUDIT_CA_FILE');p=start();token=login()
   code,result=call('/api/audit/forwarding',{'name':'Untrusted CA','url':receiver_url+'/untrusted','auth_header':secret},token);untrusted=result['data']['id'];assert change(untrusted,'test')[0]==202
   wait(lambda:dest(untrusted)['last_error']=='tls_or_transport_failed','Untrusted CA accepted')
   with lock:assert not any(row['path']=='/untrusted' for row in received)
   assert change(untrusted,'delete')[0]==200
   # Old plaintext settings are preserved but never used by this sender.
   stop(p);legacy=d/'auth/audit_forwarding.json';legacy.write_text(json.dumps([{'enabled':True,'auth_header':'OLD_SECRET','host':'localhost'}]));saved=legacy.read_bytes();p=start();token=login();assert status()['legacy_requires_review'];assert legacy.read_bytes()==saved
   print('PASS: HTTPS certificate/hostname verification, no redirect following, redacted encrypted credential, durable stable-ID restart retry, truthful 503/302 failures, paused delivery, automatic enabled delivery, disabled legacy preservation'+(', browser create/probe/delete' if os.environ.get('NABIMAN_FORWARD_BROWSER') else '')+'.')
  finally:stop(p);receiver.shutdown();receiver.server_close()
