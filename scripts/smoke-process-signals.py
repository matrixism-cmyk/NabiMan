#!/usr/bin/env python3
"""Signal only explicitly created child fixtures; never target production processes."""
import http.client,json,os,pathlib,socket,subprocess,tempfile,time,uuid
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/release/nabiman-server')).resolve()
os.umask(0o077)
with tempfile.TemporaryDirectory(prefix='nb-process-fixture-') as temporary:
 d=pathlib.Path(temporary);password='fixture-'+uuid.uuid4().hex;token=None;children=[]
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 base=f'http://127.0.0.1:{port}'
 env={k:v for k,v in os.environ.items() if not k.startswith('NABIMAN_') and k not in ['TMUX','SSH_AUTH_SOCK','KUBECONFIG']}
 env.update(NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(d/'auth'),NABIMAN_MEC_DB=str(d/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD=password,TMUX_TMPDIR=str(d/'tmux'))
 def call(path,body=None):
  c=http.client.HTTPConnection('127.0.0.1',port,timeout=15)
  try:
   c.request('POST' if body is not None else 'GET',path,None if body is None else json.dumps(body),{'Content-Type':'application/json',**({'Authorization':'Bearer '+token} if token else {})});r=c.getresponse();raw=r.read();return r.status,json.loads(raw) if raw.startswith(b'{') else None
  finally:c.close()
 def target(ignore=False):
  source="import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);print('ready',flush=True)\nwhile True: sum(range(200000));time.sleep(.01)" if ignore else "import time;print('ready',flush=True);time.sleep(60)"
  p=subprocess.Popen(['/usr/bin/python3','-c',source],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True);children.append(p);assert p.stdout.readline()=='ready\n';return p
 def identity(p):assert p in children;code,v=call('/api/processes/inspect/'+str(p.pid));assert code==200;return v['data']['identity']
 def send(p,name,reviewed):
  assert p in children and p.poll() is None
  return call('/api/processes/kill',{'pid':p.pid,'signal':name,'expected_identity':reviewed})
 with (d/'server.log').open('w') as log:
  server=subprocess.Popen([str(BINARY)],cwd=d,env=env,stdout=log,stderr=log)
  try:
   for _ in range(600):
    assert server.poll() is None,'Process fixture server exited'
    try:
     if call('/')[0]==200:break
    except OSError:pass
    time.sleep(.1)
   else:raise AssertionError('Fixture startup timeout')
   code,v=call('/api/auth/login',{'username':'admin','password':password});assert code==200;token=v['data']['token']
   assert call('/api/processes/options')[1]['data']['can_signal']
   victim=target();reviewed=identity(victim)
   assert call('/api/processes/kill',{'pid':victim.pid,'signal':'TERM'})[0]==400
   assert send(victim,'KILL','0'*64)[0]==409 and victim.poll() is None
   # These identifiers are known protected system/self/parent targets; no signal is dispatched.
   for pid in [0,1,server.pid,os.getpid()]:assert call('/api/processes/kill',{'pid':pid,'signal':'KILL','expected_identity':'0'*64})[0]==409
   code,v=send(victim,'TERM',reviewed);assert code==200 and v['data']['accepted'];victim.wait(timeout=5);assert v['data']['observation']=='exited'
   stubborn=target(True);reviewed=identity(stubborn)
   code,v=send(stubborn,'TERM',reviewed);assert code==200 and v['data']['observation']=='still_present' and stubborn.poll() is None
   result=subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-process-signals.js')],input=json.dumps({'base':base,'token':token,'pid':stubborn.pid,'chromiumPath':os.environ.get('NABIMAN_CHROMIUM')}),capture_output=True,text=True,timeout=60,cwd=ROOT/'frontend')
   assert result.returncode==0,result.stderr[-1800:] or 'Process browser fixture failed'
   assert stubborn.poll() is None
   code,v=send(stubborn,'KILL',identity(stubborn));assert code==200 and v['data']['accepted'];stubborn.wait(timeout=5)
   print(json.dumps({'passed':True,'stale_identity_and_legacy_request_rejected':True,'self_parent_system_pids_protected':True,'owned_child_term_exit_observed':True,'ignored_term_not_reported_exited':True,'owned_child_kill_verified':True,'browser':json.loads(result.stdout),'limits':['Signals target only fixture children; no actual PID reuse forced','Exit observed within 100ms is distinct from graceful shutdown or service recovery']}))
  finally:
   for child in children:
    if child.poll() is None:child.kill()
    child.wait(timeout=5)
   server.terminate()
   try:server.wait(timeout=10)
   except subprocess.TimeoutExpired:server.kill();server.wait()
