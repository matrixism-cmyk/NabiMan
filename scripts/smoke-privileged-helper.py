#!/usr/bin/env python3
"""Real unprivileged API/root helper/Unix credentials/SQLite faults; only a nonexistent fixture unit."""
import json, os, pathlib, signal, socket, sqlite3, struct, subprocess, tempfile, time, uuid
from concurrent.futures import ThreadPoolExecutor
from urllib.request import Request, urlopen
from urllib.error import HTTPError
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/debug/nabiman-server'))
HELPER=BINARY.with_name('nabiman-privileged-helper')
assert os.geteuid()!=0,'Run the API fixture as a non-root user with sudo -n access'
subprocess.run(['sudo','-n','true'],check=True)
assert HELPER.is_file(),'Build both binaries first'
fixture='/run/nabiman-helper-smoke-'+uuid.uuid4().hex
unit='nabiman-nonexistent-fixture-'+uuid.uuid4().hex+'.service'
subprocess.run(['sudo','-n','mkdir','-m','0755',fixture],check=True)
helper_path=fixture+'/helper';policy_path=fixture+'/policy.json';socket_path=fixture+'/control.sock';ledger_path=fixture+'/operations.db'
subprocess.run(['sudo','-n','install','-m','0755',str(HELPER),helper_path],check=True)
policy={'api_uid':os.geteuid(),'services':[{'unit':unit,'actions':['restart']}]}
def root_code(code, payload=None):
    run=subprocess.run(['sudo','-n','python3','-c',code,fixture],input=json.dumps(payload or {}),text=True,capture_output=True)
    assert run.returncode==0,run.stderr[-1000:]
    return json.loads(run.stdout) if run.stdout.strip() else None
def save_policy(value):
    root_code('import json,os,sys; p=sys.argv[1]+"/policy.json"; value=json.load(sys.stdin); fd=os.open(p,os.O_WRONLY|os.O_CREAT|os.O_TRUNC,0o600); os.write(fd,json.dumps(value).encode()); os.fsync(fd); os.close(fd)',value)
def ledger(sql,parameters=()):
    return root_code('import json,sqlite3,sys; value=json.load(sys.stdin); db=sqlite3.connect(sys.argv[1]+"/operations.db"); rows=db.execute(value["sql"],value["parameters"]).fetchall(); db.commit(); print(json.dumps(rows))',{'sql':sql,'parameters':parameters})
def wire(value):
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as stream:
        stream.settimeout(5);stream.connect(socket_path);data=json.dumps(value).encode();stream.sendall(struct.pack('!I',len(data))+data)
        header=stream.recv(4)
        if not header:return None
        length=struct.unpack('!I',header)[0];assert length<=4096
        body=b''
        while len(body)<length:
            chunk=stream.recv(length-len(body));assert chunk;body+=chunk
        return json.loads(body)
def browser_smoke(script,payload):
    browser=subprocess.Popen(['node',str(ROOT/'frontend/scripts'/script)],cwd=ROOT/'frontend',stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
    try:stdout,stderr=browser.communicate(json.dumps(payload),timeout=60)
    except subprocess.TimeoutExpired:
        os.killpg(browser.pid,signal.SIGTERM)
        try:browser.communicate(timeout=5)
        except subprocess.TimeoutExpired:os.killpg(browser.pid,signal.SIGKILL);browser.communicate(timeout=5)
        raise AssertionError('Own helper browser fixture timed out')
    assert browser.returncode==0,stderr[-2000:]
    return json.loads(stdout)
def command(actor='fixture-actor',action='restart',target=None):
    return {'version':1,'request_id':str(uuid.uuid4()),'actor_id':actor,'expires_at':int(time.time())+3,'operation':{'kind':'execute','unit':target or unit,'action':action}}
def count():return ledger('SELECT count(*) FROM helper_operations')[0][0]
helper=None;helper_pid=None;api=None
with tempfile.TemporaryDirectory(prefix='nabiman-helper-api-') as temporary:
    directory=pathlib.Path(temporary)
    def start_helper():
        global helper,helper_pid
        launcher='import subprocess,sys; p=subprocess.Popen(sys.argv[1:]); print(p.pid,flush=True); sys.exit(p.wait())'
        approved_pid=api.pid if api and api.poll() is None else os.getpid()
        subprocess.run(['sudo','-n',helper_path,'--policy',policy_path,'--socket',socket_path,'--pin-api',str(approved_pid)],check=True,capture_output=True)
        error=(directory/'helper.log').open('a')
        helper=subprocess.Popen(['sudo','-n','python3','-c',launcher,helper_path,'--policy',policy_path,'--socket',socket_path,'--ledger',ledger_path],stdout=subprocess.PIPE,stderr=error,text=True)
        helper_pid=int(helper.stdout.readline());error.close()
        for _ in range(100):
            assert helper.poll() is None,(directory/'helper.log').read_text()[-1500:]
            if pathlib.Path(socket_path).exists():
                try:
                    probe=command();probe['operation']={'kind':'inspect','operation_id':str(uuid.uuid4())}
                    proof=wire(probe)
                    if proof and proof['status']==('rejected' if api and api.poll() is None else 'not_found'):return
                except (OSError,AssertionError):pass
            time.sleep(.05)
        raise AssertionError('Helper fixture startup timeout')
    def stop_helper():
        global helper,helper_pid
        if helper and helper.poll() is None:
            # Signal only this exact child, after verifying its executable path.
            root_code('import json,os,signal,sys; v=json.load(sys.stdin); p="/proc/"+str(v["pid"])+"/cmdline"; args=open(p,"rb").read().split(b"\\0"); assert args[0].decode()==sys.argv[1]+"/helper"; os.kill(v["pid"],signal.SIGTERM)',{'pid':helper_pid})
            helper.wait(timeout=10)
        helper=None;helper_pid=None
    try:
        save_policy(policy);start_helper()
        first=command();reply=wire(first)
        assert reply['status']=='completed' and reply['receipt']['state']=='command_failed'
        assert reply['receipt']['exit_code']!=0 and count()==1
        assert wire(first)['status']=='conflict' and count()==1
        assert wire(command(target='ssh.service'))['status']=='rejected'
        assert wire(command(action='stop'))['status']=='rejected' and count()==1
        overflow=command();overflow['expires_at']=-(2**63)
        assert wire(overflow)['status']=='expired' and helper.poll() is None
        malformed=command();malformed['operation']['program']='/bin/sh'
        assert wire(malformed) is None and count()==1 and helper.poll() is None
        assert pathlib.Path(socket_path).stat().st_uid==os.geteuid() and pathlib.Path(socket_path).stat().st_mode&0o777==0o600
        peer_probe='import json,socket,struct,sys; v=json.load(sys.stdin); s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]+"/control.sock"); b=json.dumps(v).encode(); s.sendall(struct.pack("!I",len(b))+b); n=struct.unpack("!I",s.recv(4))[0]; print(s.recv(n).decode())'
        assert root_code(peer_probe,command())['status']=='rejected' and count()==1
        second=subprocess.run(['sudo','-n',helper_path,'--policy',policy_path,'--socket',socket_path,'--ledger',ledger_path],capture_output=True,timeout=10)
        assert second.returncode!=0 and helper.poll() is None
        print('PASS: real root helper authenticates kernel UID, exact service/action policy, bounded strict messages, private socket, duplicate writer rejection and no duplicate dispatch.',flush=True)
        with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
        mock_bin=directory/'bin';mock_bin.mkdir();marker=directory/'direct-dispatch'
        fake=mock_bin/'systemctl';fake.write_text('#!/bin/sh\nif [ "$1" = restart ]; then printf attempted > "$NABIMAN_FIXTURE_DISPATCH_MARKER"; fi\nexit 1\n');fake.chmod(0o755)
        env=dict(os.environ,NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(directory/'auth'),NABIMAN_MEC_DB=str(directory/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD='isolated-helper-password-123',NABIMAN_HELPER_SOCKET=socket_path,PATH=str(mock_bin)+':'+os.environ['PATH'],NABIMAN_FIXTURE_DISPATCH_MARKER=str(marker))
        base=f'http://127.0.0.1:{port}';token=None
        def http(path,body=None,key=None,bearer=None):
            headers={'Content-Type':'application/json'}
            if bearer or token:headers['Authorization']='Bearer '+(bearer or token)
            if key:headers['Idempotency-Key']=key
            try:response=urlopen(Request(base+path,None if body is None else json.dumps(body).encode(),headers),timeout=30)
            except HTTPError as error:response=error
            with response:return response.status,json.load(response),response.headers
        log=(directory/'api.log').open('w');api=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log);log.close()
        for _ in range(100):
            assert api.poll() is None,'API fixture exited'
            try:
                code,value,_=http('/api/auth/login',{'username':'admin','password':env['NABIMAN_PASSWORD']})
                if code==200:token=value['data']['token'];break
            except OSError:pass
            time.sleep(.1)
        else:raise AssertionError('API fixture startup timeout')
        subprocess.run(['sudo','-n',helper_path,'--policy',policy_path,'--socket',socket_path,'--pin-api',str(api.pid)],check=True,capture_output=True)
        assert wire(command())['status']=='rejected' and count()==1
        # Dumpability changes the files inside /proc/PID, not the directory owner.
        assert pathlib.Path(f'/proc/{api.pid}/mem').stat().st_uid==0
        try:
            with open(f'/proc/{api.pid}/mem','rb'):pass
        except PermissionError:pass
        else:raise AssertionError('Same-UID parent could open API process memory')
        own=str(uuid.uuid4());body={'name':unit.removesuffix('.service'),'action':'restart'}
        code,value,headers=http('/api/services/action',body,own)
        assert code==502 and value['data']['receipt']['state']=='command_failed' and headers['Cache-Control']=='no-store'
        assert count()==2 and not marker.exists()
        assert http('/api/services/action',body,own)[0]==409 and count()==2
        code,value,_=http('/api/services/operations/'+own);assert code==200 and value['success'] and value['data']['receipt']['state']=='command_failed'
        assert http('/api/services/operations/'+first['request_id'])[0]==404
        assert http('/api/services/action',body,'not-a-uuid')[0]==400
        assert http('/api/services/action',{'name':'--all','action':'restart'},str(uuid.uuid4()))[0]==400
        assert http('/api/services/action',{'name':'ssh','action':'restart'},str(uuid.uuid4()))[0]==403 and count()==2
        http('/api/auth/register',{'username':'viewer','password':'isolated-viewer-password-123'})
        users=http('/api/users')[1]['data'];viewer=next(u for u in users if u['username']=='viewer')
        assert http('/api/users/'+viewer['id']+'/review',{'status':'active','memberships':[]})[0]==200
        viewer_token=http('/api/auth/login',{'username':'viewer','password':'isolated-viewer-password-123'})[1]['data']['token']
        assert http('/api/services/action',body,str(uuid.uuid4()),viewer_token)[0]==403 and count()==2
        print('PASS: actual non-root API pin excludes another same-UID process and blocks same-UID process memory access; authenticated role boundaries, correlated durable command receipts, same-key 409, own-only read-only inspection and no direct PATH execution.',flush=True)
        observed=browser_smoke('smoke-service-control.js',{'base':base,'token':token,'unit':unit})
        assert observed['posts']==2 and count()==4
        for operation in [observed['confirmed'],observed['lost']]:
            assert ledger('SELECT state FROM helper_operations WHERE id=?',[operation])==[['command_failed']]
        print('PASS: actual service UI sends once, displays failed command distinctly, recovers a lost response after reload by read-only inspection, retains an account-specific ID and never replays.',flush=True)
        races=browser_smoke('smoke-auth-session.js',{'base':base,'adminPassword':env['NABIMAN_PASSWORD'],'viewerPassword':'isolated-viewer-password-123'})
        assert races=={'scenarios':3,'replayed':0} and count()==4
        print('PASS: actual browser logout and replacement login reject delayed 401/refresh success/refresh failure without credential overwrite, logout of the new account or mutation replay.',flush=True)
        # Hold a shared DB lock so admission passes but the intent cannot commit.
        reader_code='import sqlite3,sys; db=sqlite3.connect(sys.argv[1]); db.execute("BEGIN"); db.execute("SELECT count(*) FROM helper_operations").fetchone(); print("ready",flush=True); sys.stdin.readline(); db.commit()'
        reader=subprocess.Popen(['sudo','-n','python3','-c',reader_code,ledger_path],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
        try:
            assert reader.stdout.readline().strip()=='ready'
            with ThreadPoolExecutor(max_workers=1) as pool:
                pending=pool.submit(http,'/api/services/action',body,str(uuid.uuid4()))
                try:
                    for _ in range(40):
                        if root_code('import json,os,sys; print(json.dumps(os.path.exists(sys.argv[1]+"/operations.db-journal")))'):break
                        time.sleep(.005)
                    else:raise AssertionError('Intent did not reach the held commit boundary')
                    subprocess.run(['sudo','-n',helper_path,'--policy',policy_path,'--socket',socket_path,'--pin-api',str(os.getpid())],check=True,capture_output=True)
                finally:
                    reader.stdin.write('release\n');reader.stdin.flush()
                code,value,_=pending.result(timeout=30)
                assert code==502 and value['data']['receipt']['state']=='not_dispatched'
        finally:
            if reader.poll() is None:
                reader.stdin.close();reader.wait(timeout=5)
            reader.stdout.close()
            subprocess.run(['sudo','-n',helper_path,'--policy',policy_path,'--socket',socket_path,'--pin-api',str(api.pid)],check=True,capture_output=True)
        baseline=count();assert baseline==5 and not marker.exists()
        print('PASS: root pin rotation while durable intent commit is blocked records not_dispatched and prevents OS dispatch.',flush=True)
        # Root policy revocation also applies to a helper already running with cached rules.
        initial_policy_hash=json.loads(ledger('SELECT receipt FROM helper_operations LIMIT 1')[0][0])['policy_hash']
        for fault in ['revoked','permissions','missing','malformed','symlink']:
            if fault=='revoked':save_policy({**policy,'services':[]})
            elif fault=='permissions':root_code('import os,sys; os.chmod(sys.argv[1]+"/policy.json",0o644)')
            elif fault=='missing':root_code('import os,sys; os.rename(sys.argv[1]+"/policy.json",sys.argv[1]+"/policy.saved")')
            elif fault=='malformed':root_code('import sys; open(sys.argv[1]+"/policy.json","w").write("{")')
            else:root_code('import os,sys; p=sys.argv[1]; os.rename(p+"/policy.json",p+"/policy.saved"); os.symlink(p+"/policy.saved",p+"/policy.json")')
            try:
                operation=str(uuid.uuid4());code,value,_=http('/api/services/action',body,operation)
                assert code==502 and value['data']['receipt']['state']=='not_dispatched',fault
                assert value['data']['receipt']['policy_hash']==initial_policy_hash
                assert http('/api/services/operations/'+operation)[1]['data']['receipt']['state']=='not_dispatched'
                assert http('/api/services/action',body,operation)[0]==409
            finally:
                if fault in ['missing','symlink']:
                    root_code('import os,sys; p=sys.argv[1]; q=p+"/policy.json"; os.path.lexists(q) and os.unlink(q); os.rename(p+"/policy.saved",q)')
                root_code('import os,sys; os.chmod(sys.argv[1]+"/policy.json",0o600)');save_policy(policy)
        # Formatting changes are not a different approval, and restoring the exact
        # approved policy allows a fresh ID without restarting or replaying old IDs.
        root_code('import json,sys; p=sys.argv[1]+"/policy.json"; v=json.load(open(p)); open(p,"w").write(json.dumps(v,indent=4,sort_keys=True))')
        assert http('/api/services/action',body,str(uuid.uuid4()))[1]['data']['receipt']['state']=='command_failed'
        print('PASS: running helper denies revoked, unsafe, missing, malformed and symlink policies; receipts stay inspectable, same IDs never replay, and semantic policy restoration admits a fresh operation.',flush=True)
        # Revoke after admission while a real SQLite reader holds the intent commit.
        reader=subprocess.Popen(['sudo','-n','python3','-c',reader_code,ledger_path],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
        try:
            assert reader.stdout.readline().strip()=='ready'
            with ThreadPoolExecutor(max_workers=1) as pool:
                pending=pool.submit(http,'/api/services/action',body,str(uuid.uuid4()))
                try:
                    for _ in range(40):
                        if root_code('import json,os,sys; print(json.dumps(os.path.exists(sys.argv[1]+"/operations.db-journal")))'):break
                        time.sleep(.005)
                    else:raise AssertionError('Policy test did not reach the held commit boundary')
                    save_policy({**policy,'services':[]})
                finally:
                    reader.stdin.write('release\n');reader.stdin.flush()
                code,value,_=pending.result(timeout=30)
                assert code==502 and value['data']['receipt']['state']=='not_dispatched'
        finally:
            if reader.poll() is None:reader.stdin.close();reader.wait(timeout=5)
            reader.stdout.close();save_policy(policy)
        baseline=count();assert baseline==12 and not marker.exists()
        print('PASS: root policy revocation after admission and before durable intent commit prevents privileged dispatch without restarting the helper.',flush=True)
        stop_helper()
        missing=str(uuid.uuid4());code,value,_=http('/api/services/action',body,missing)
        assert code==503 and not marker.exists() and count()==baseline
        start_helper()
        ledger("CREATE TRIGGER reject_intent BEFORE INSERT ON helper_operations BEGIN SELECT RAISE(ABORT,'fixture intent failure'); END")
        code,value,_=http('/api/services/action',body,str(uuid.uuid4()));assert code==502
        helper.wait(timeout=10);assert helper.returncode!=0 and count()==baseline
        ledger('DROP TRIGGER reject_intent');helper=None;start_helper()
        ledger("CREATE TRIGGER reject_confirm BEFORE UPDATE ON helper_operations BEGIN SELECT RAISE(ABORT,'fixture confirmation failure'); END")
        unknown=str(uuid.uuid4());code,value,_=http('/api/services/action',body,unknown);assert code==502
        helper.wait(timeout=10);assert helper.returncode!=0 and count()==baseline+1
        ledger('DROP TRIGGER reject_confirm');helper=None;start_helper()
        code,value,_=http('/api/services/operations/'+unknown);assert code==200 and value['data']['receipt']['state']=='unknown_result'
        code,value,_=http('/api/services/action',body,str(uuid.uuid4()));assert code==409 and value['data']['operation_id']==unknown and count()==baseline+1
        assert not marker.exists()
        print('PASS: unavailable helper has no fallback; real SQLite intent/confirmation faults halt helper, restart marks running unknown, inspection does not replay and unresolved unit rejects a fresh key.',flush=True)
        stop_helper()
        policy['services'][0]['actions'].append('stop');save_policy(policy)
        start_helper()
        code,value,_=http('/api/services/action',{'name':unit,'action':'stop'},str(uuid.uuid4()))
        assert code==409 and value['data']['operation_id']==unknown and count()==baseline+1
        stop_helper();save_policy({**policy,'api_uid':0})
        invalid=subprocess.run(['sudo','-n',helper_path,'--policy',policy_path,'--socket',socket_path,'--ledger',ledger_path],capture_output=True,timeout=10)
        assert invalid.returncode!=0
        save_policy(policy)
        root_code('import os,sys; os.chmod(sys.argv[1]+"/policy.json",0o644)')
        unsafe=subprocess.run(['sudo','-n',helper_path,'--policy',policy_path,'--socket',socket_path,'--ledger',ledger_path],capture_output=True,timeout=10)
        assert unsafe.returncode!=0
        root_code('import os,sys; os.chmod(sys.argv[1]+"/policy.json",0o600); os.rename(sys.argv[1]+"/operations.db",sys.argv[1]+"/reviewed.db"); fd=os.open(sys.argv[1]+"/operations.db",os.O_WRONLY|os.O_CREAT,0o600); os.write(fd,b"fixture corruption"); os.close(fd)')
        corrupt=subprocess.run(['sudo','-n',helper_path,'--policy',policy_path,'--socket',socket_path,'--ledger',ledger_path],capture_output=True,timeout=10)
        assert corrupt.returncode!=0
        assert root_code('import json,sys; print(json.dumps(open(sys.argv[1]+"/operations.db","rb").read().decode()))')=='fixture corruption'
        root_code('import os,sys; os.unlink(sys.argv[1]+"/operations.db")')
        missing=subprocess.run(['sudo','-n',helper_path,'--policy',policy_path,'--socket',socket_path,'--ledger',ledger_path],capture_output=True,timeout=10)
        assert missing.returncode!=0 and not pathlib.Path(ledger_path).exists()
        print('PASS: root API UID, non-private policy, corrupt ledger and established missing ledger are rejected before listener admission; corruption is preserved.',flush=True)
    finally:
        if api and api.poll() is None:
            api.terminate()
            try:api.wait(timeout=10)
            except subprocess.TimeoutExpired:api.kill();api.wait(timeout=5)
        stop_helper()
        subprocess.run(['sudo','-n','rm','-rf','--',fixture],check=True)
