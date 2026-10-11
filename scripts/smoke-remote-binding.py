#!/usr/bin/env python3
"""Real loopback SSH protocol + isolated server and browser; no production targets."""
import base64, hashlib, json, os, pathlib, socket, subprocess, tempfile, threading, time
from urllib.request import Request, urlopen
from urllib.error import HTTPError
import paramiko
ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY', ROOT/'backend/target/debug/nabiman-server'))
class SSHFixture(paramiko.ServerInterface):
    def __init__(self): self.shell = threading.Event()
    def check_auth_none(self, username): return paramiko.AUTH_SUCCESSFUL
    def check_channel_request(self, kind, channel_id): return paramiko.OPEN_SUCCEEDED if kind == 'session' else paramiko.OPEN_FAILED_ADMINISTRATIVELY_PROHIBITED
    def check_channel_pty_request(self, *args): return True
    def check_channel_shell_request(self, channel): self.shell.set(); return True
with tempfile.TemporaryDirectory(prefix='nabiman-remote-binding-') as temporary:
    directory = pathlib.Path(temporary)
    ssh_listener = socket.socket(); ssh_listener.bind(('127.0.0.1',0)); ssh_listener.listen(); ssh_listener.settimeout(.2)
    key = paramiko.RSAKey.generate(2048)
    public = key.get_name()+' '+key.get_base64()
    fingerprint = 'SHA256:'+base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip('=')
    stop = threading.Event(); transports = []
    def serve(connection):
        transport = paramiko.Transport(connection); transports.append(transport)
        try:
            transport.add_server_key(key); interface=SSHFixture(); transport.start_server(server=interface)
            channel = transport.accept(10)
            if channel is None or not interface.shell.wait(10): return
            channel.sendall(b'REMOTE-READY\r\n')
            while not stop.is_set():
                data=channel.recv(8192)
                if not data: break
                channel.sendall(b'REMOTE-ECHO:'+data.replace(b'\n',b'\r\n'))
        except (EOFError, OSError, paramiko.SSHException): pass
        finally: transport.close()
    def accept():
        while not stop.is_set():
            try: connection,_=ssh_listener.accept()
            except socket.timeout: continue
            except OSError: break
            threading.Thread(target=serve,args=(connection,),daemon=True).start()
    threading.Thread(target=accept,daemon=True).start()
    with socket.socket() as free:
        free.bind(('127.0.0.1',0)); port=free.getsockname()[1]
    (directory/'tmux').mkdir()
    env=dict(os.environ,NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(directory/'auth'),NABIMAN_MEC_DB=str(directory/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD='isolated-binding-password-123',TMUX_TMPDIR=str(directory/'tmux'))
    token=None
    def request(path, body=None, method=None):
        headers={'Content-Type':'application/json'}
        if token: headers['Authorization']='Bearer '+token
        req=Request(f'http://127.0.0.1:{port}'+path,None if body is None else json.dumps(body).encode(),headers,method=method)
        try: response=urlopen(req,timeout=10)
        except HTTPError as error: response=error
        with response: return response.status,json.loads(response.read())
    with (directory/'server.log').open('w') as log:
        server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log)
        try:
            for _ in range(100):
                assert server.poll() is None, 'Fixture startup failed'
                try:
                    code,value=request('/api/auth/login',{'username':'admin','password':env['NABIMAN_PASSWORD']})
                    if code==200 and value.get('success'): token=value['data']['token']; break
                except OSError: pass
                time.sleep(.1)
            else: raise AssertionError('Login timeout')
            code,value=request('/api/remote-servers',{'name':'Loopback SSH','host':'127.0.0.1','port':ssh_listener.getsockname()[1],'user':'test','auth_method':'key'})
            assert code==200 and value['success']; record=value['data']; identifier=record['id']
            code,value=request('/api/remote-servers/'+identifier+'/host-trust',{'host':record['host'],'port':record['port'],'public_key':public,'fingerprint':fingerprint,'previous_fingerprint':'','previous_version':record['host_trust_version'],'reason':'Generated isolated server key'},'PUT')
            assert code==200 and value['success']
            node=subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-remote-binding.js')],input=json.dumps({'base':f'http://127.0.0.1:{port}','token':token,'server_id':identifier}),text=True,capture_output=True,timeout=60)
            assert node.returncode==0,node.stderr
            print(node.stdout.strip())
            sessions=json.loads((directory/'auth/terminal_sessions.json').read_text())
            bound=[s for s in sessions.values() if s.get('remote_binding')]
            assert len(bound)>=2 and all(s['remote_binding']['server_id']==identifier for s in bound)
            assert len({s['remote_binding']['approval_version'] for s in bound})==2
            print('PASS: session metadata stores exact remote target and distinct approval generations; real SSH uses the verified loopback key.')
            current_version=json.loads((directory/'auth/remote_servers.json').read_text())[0]['host_trust_version']
            old_id=next(i for i,s in sessions.items() if s.get('remote_binding') and s['remote_binding']['approval_version']!=current_version)
            current_id=next(i for i,s in sessions.items() if s.get('remote_binding') and s['remote_binding']['approval_version']==current_version)
            server.terminate(); server.wait(timeout=10)
            server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log)
            token=None
            for _ in range(100):
                assert server.poll() is None, 'Restart failed'
                try:
                    code,value=request('/api/auth/login',{'username':'admin','password':env['NABIMAN_PASSWORD']})
                    if code==200 and value.get('success'): token=value['data']['token']; break
                except OSError: pass
                time.sleep(.1)
            else: raise AssertionError('Restart login timeout')
            node=subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-remote-binding.js')],input=json.dumps({'base':f'http://127.0.0.1:{port}','token':token,'server_id':identifier,'resume':{'old':old_id,'current':current_id}}),text=True,capture_output=True,timeout=60)
            assert node.returncode==0,node.stderr
            print('PASS: actual restart denies previous approval, restores current approved remote attachment and closes it with 1008 after target deletion.')

        finally:
            server.terminate()
            try: server.wait(timeout=10)
            except subprocess.TimeoutExpired: server.kill(); server.wait(timeout=5)
            registry=directory/'auth/terminal_sessions.json'
            if registry.exists():
                for session in json.loads(registry.read_text()).values():
                    name=session.get('tmux_name','')
                    if name.startswith('nb_admin_'): subprocess.run(['tmux','kill-session','-t',name],env=env,capture_output=True)
            stop.set(); ssh_listener.close()
            for transport in transports: transport.close()
