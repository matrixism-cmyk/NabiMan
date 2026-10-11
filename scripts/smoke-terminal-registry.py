#!/usr/bin/env python3
"""Fail a live terminal metadata write in an isolated server; verify no success and reviewed recovery."""
import json, os, pathlib, shlex, shutil, signal, socket, subprocess, tempfile, time
from urllib.request import Request, urlopen
from urllib.error import HTTPError
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(os.environ.get('NABIMAN_SMOKE_BINARY',ROOT/'backend/target/debug/nabiman-server'))
NODE=r'''
const fs=require('fs');const {chromium}=require('playwright');
(async()=>{const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,args:['--no-sandbox']});try{
const page=await browser.newPage();await page.goto(f.base);
const session=await page.evaluate(async f=>{const r=await fetch('/api/terminal/tickets',{method:'POST',headers:{Authorization:'Bearer '+f.token,'Content-Type':'application/json'},body:JSON.stringify({query:''})});const value=await r.json();const ws=new WebSocket(location.origin.replace(/^http/,'ws')+'/api/terminal?ticket='+value.data.ticket);window.fixtureSocket=ws;let ended=false;window.fixtureEOF=new Promise(resolve=>{window.resolveFixtureEOF=resolve;});return await new Promise((resolve,reject)=>{ws.onmessage=e=>{if(typeof e.data!=='string')return;if(e.data.startsWith('\x02END'))ended=true;if(e.data.startsWith('\x02SESSION:'))resolve(e.data.slice(9));};ws.onerror=()=>reject(new Error('Fixture terminal failed'));ws.onclose=()=>{window.resolveFixtureEOF({closed:true,ended});reject(new Error('Fixture terminal closed'));};setTimeout(()=>reject(new Error('Fixture session timeout')),10000);});},f);
console.log(JSON.stringify({session_id:session}));if(f.expectEOF){const closed=await page.evaluate(()=>Promise.race([window.fixtureEOF,new Promise((_,reject)=>setTimeout(()=>reject(new Error('Fixture EOF timeout')),15000))]));console.log(JSON.stringify(closed));}else await page.waitForTimeout(15000);
}finally{await browser.close();}})().catch(()=>{console.error('Fixture browser failed');process.exit(1);});
'''
NODE_DENIED=r'''
const fs=require('fs');const {chromium}=require('playwright');
(async()=>{const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,args:['--no-sandbox']});try{
const page=await browser.newPage();await page.goto(f.base);
await page.evaluate(async f=>{await Promise.all([0,1].map(async()=>{
const r=await fetch('/api/terminal/tickets',{method:'POST',headers:{Authorization:'Bearer '+f.token,'Content-Type':'application/json'},body:JSON.stringify({query:''})});const value=await r.json();
if(!r.ok||!value.success)throw new Error('Ticket request failed');
const ws=new WebSocket(location.origin.replace(/^http/,'ws')+'/api/terminal?ticket='+value.data.ticket);
await new Promise((resolve,reject)=>{let notice=false,ended=false;const timer=setTimeout(()=>reject(new Error('Denial timeout')),10000);
ws.onmessage=e=>{if(typeof e.data!=='string')return;if(e.data.startsWith('\x02SESSION:'))reject(new Error('Unexpected OS session admission'));if(e.data.includes(f.expectText||'Terminal capacity'))notice=true;if(e.data.startsWith('\x02END'))ended=true;};
ws.onerror=()=>reject(new Error('Denial failed'));ws.onclose=()=>{clearTimeout(timer);if(notice&&ended)resolve();else reject(new Error('Expected capacity notice and end'));};
});}));},f);
}finally{await browser.close();}})().catch(()=>{console.error('Concurrent admission denial failed');process.exit(1);});
'''
with tempfile.TemporaryDirectory(prefix='nabiman-terminal-registry-') as temporary:
    directory=pathlib.Path(temporary);(directory/'tmux').mkdir()
    with socket.socket() as sock: sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    env=dict(os.environ,NABIMAN_PORT=str(port),NABIMAN_DATA_DIR=str(directory/'auth'),NABIMAN_MEC_DB=str(directory/'mec.db'),NABIMAN_MEC_MODE='mock',NABIMAN_STATIC=str(ROOT/'frontend/build'),NABIMAN_PASSWORD='isolated-registry-password-123',TMUX_TMPDIR=str(directory/'tmux'))
    token=None
    def request(path,body=None,method=None):
        headers={'Content-Type':'application/json'}
        if token:headers['Authorization']='Bearer '+token
        req=Request(f'http://127.0.0.1:{port}'+path,None if body is None else json.dumps(body).encode(),headers,method=method)
        try: response=urlopen(req,timeout=10)
        except HTTPError as error:response=error
        with response:return response.status,json.loads(response.read())
    def login(server):
        for _ in range(100):
            assert server.poll() is None,'Fixture server exited'
            try:
                code,value=request('/api/auth/login',{'username':'admin','password':env['NABIMAN_PASSWORD']})
                if code==200 and value.get('success'):return value['data']['token']
            except OSError:pass
            time.sleep(.1)
        raise AssertionError('Fixture login timeout')
    browser=None
    with (directory/'server.log').open('w') as log:
        server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log)
        try:
            token=login(server)
            browser=subprocess.Popen(['node','-e',NODE],cwd=ROOT/'frontend',stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
            browser.stdin.write(json.dumps({'base':f'http://127.0.0.1:{port}','token':token}));browser.stdin.close()
            identifier=json.loads(browser.stdout.readline())['session_id']
            assert identifier.startswith('pty_')
            code,value=request('/api/terminal/settings')
            assert code==200 and value['data']['font_size']==14
            settings={**value['data'],'font_size':19,'keepalive':False,'idle_timeout_secs':120}
            code,value=request('/api/terminal/settings',settings,method='PUT')
            assert code==200 and value['success'] and value['data']==settings
            path=directory/'auth/terminal_sessions.json';saved=directory/'auth/reviewed-terminal-backup.json'
            original=path.read_bytes();session=json.loads(original)[identifier]
            assert path.stat().st_mode&0o777==0o600 and session['title']==''
            path.replace(saved);path.mkdir()
            try:request('/api/terminal/sessions/'+identifier+'/name',{'name':'unacknowledged change'})
            except OSError:pass
            else:raise AssertionError('Failed terminal publication returned a receipt')
            assert server.wait(timeout=10)==79,'Live server did not stop on failed terminal publication'
            assert saved.read_bytes()==original and path.is_dir()
            users_before=(directory/'auth/users.json').read_bytes()
            server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log)
            assert server.wait(timeout=10)!=0,'Invalid terminal metadata was accepted on restart'
            assert (directory/'auth/users.json').read_bytes()==users_before,'Validation failure changed auth state'
            # Explicit fixture operator recovery, never an automatic server rollback.
            path.rmdir();saved.replace(path)
            server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log);token=None;token=login(server)
            code,value=request('/api/terminal/sessions')
            restored=next(s for s in value['data'] if s['id']==identifier)
            assert code==200 and restored['alive'] and restored['title']==''
            code,value=request('/api/terminal/settings')
            assert code==200 and value['data']==settings
            print('PASS: real terminal write failure returns no HTTP receipt, exits 79, preserves reviewed snapshot, blocks invalid restart before auth changes and restores original live shell after explicit fixture recovery.')
            server.terminate();server.wait(timeout=10)
            missing_tools=directory/'missing-tools';missing_tools.mkdir()
            server=subprocess.Popen([str(BINARY)],env={**env,'PATH':str(missing_tools)},cwd=directory,stdout=log,stderr=log);token=None;token=login(server)
            code,value=request('/api/terminal/sessions')
            unknown=next(s for s in value['data'] if s['id']==identifier)
            assert unknown['state']=='unknown' and not unknown['alive'] and not unknown['preview']
            before_unknown=path.read_bytes()
            code,value=request('/api/terminal/sessions/'+identifier,method='DELETE')
            assert code==503 and not value['success'] and path.read_bytes()==before_unknown
            server.terminate();server.wait(timeout=10)
            server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log);token=None;token=login(server)
            code,value=request('/api/terminal/sessions')
            assert next(s for s in value['data'] if s['id']==identifier)['state']=='alive'
            print('PASS: unavailable tmux is unknown, never empty/dead success; metadata survives lookup and termination failure and recovers when the tool returns.')

            server.terminate();server.wait(timeout=10)
            # A valid large legacy snapshot fits on disk, but has no space for
            # another admission plus future metadata growth. Preserve the real
            # underlying shell and restore the reviewed snapshot explicitly.
            reviewed=path.read_bytes();large=json.loads(reviewed)
            large[identifier]['ssh_cmd']=''
            overhead=len(json.dumps(large,separators=(',',':')).encode())
            large[identifier]['ssh_cmd']='x'*(16*1024*1024-overhead)
            path.write_text(json.dumps(large,separators=(',',':')))
            assert path.stat().st_size==16*1024*1024
            server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log);token=None;token=login(server)
            before_denial=path.read_bytes()
            assert len(before_denial)==16*1024*1024
            before_tmux=subprocess.run(['tmux','list-sessions','-F','#{session_name}'],env=env,capture_output=True,check=True).stdout
            denied=subprocess.run(['node','-e',NODE_DENIED],cwd=ROOT/'frontend',input=json.dumps({'base':f'http://127.0.0.1:{port}','token':token}),text=True,capture_output=True,timeout=30)
            assert denied.returncode==0,'Concurrent admissions did not return capacity notices'
            assert server.poll() is None and path.read_bytes()==before_denial
            after_tmux=subprocess.run(['tmux','list-sessions','-F','#{session_name}'],env=env,capture_output=True,check=True).stdout
            assert after_tmux==before_tmux,'Rejected admissions created an OS session'
            code,value=request('/api/terminal/sessions')
            assert code==200 and len(value['data'])==1
            name=large[identifier]['tmux_name']
            def window_name():
                return subprocess.run(['tmux','display-message','-p','-t','='+name+':','#{window_name}'],env=env,capture_output=True,check=True).stdout
            previous_window=window_name()
            for action,body in [('name',{'name':'한'*60}),('timeout',{'timeout_secs':30*24*3600}),('keepalive',{'keepalive':False})]:
                code,value=request('/api/terminal/sessions/'+identifier+'/'+action,body)
                assert code==409 and not value['success'], 'Oversized metadata edit was not rejected'
                assert server.poll() is None and path.read_bytes()==before_denial
            assert window_name()==previous_window, 'Rejected rename changed tmux window'
            code,value=request('/api/terminal/sessions')
            current=next(s for s in value['data'] if s['id']==identifier)
            assert current['title']==large[identifier]['title'] and current['timeout_secs']==large[identifier]['timeout_secs'] and current['keepalive']==large[identifier]['keepalive']
            code,value=request('/api/terminal/sessions/'+identifier+'/share',{'shared':True})
            assert code==200 and value['success'] and path.stat().st_size==16*1024*1024-1
            code,value=request('/api/terminal/sessions/'+identifier+'/share',{'shared':False})
            assert code==200 and value['success'] and path.stat().st_size==16*1024*1024
            print('PASS: oversized title/timeout/keepalive edits return 409 without memory, disk or tmux window changes; shrinking edits remain allowed at full capacity.')
            server.terminate();server.wait(timeout=10);path.write_bytes(reviewed)
            server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log);token=None;token=login(server)
            code,value=request('/api/terminal/sessions')
            assert next(s for s in value['data'] if s['id']==identifier)['state']=='alive'
            print('PASS: concurrent capacity denials return notices without creating tmux sessions, changing metadata or stopping the server; reviewed original shell remains available.')

            if browser.poll() is None:
                os.killpg(browser.pid,signal.SIGTERM);browser.wait(timeout=5)
            server.terminate();server.wait(timeout=10)
            tools=directory/'probe-tools';tools.mkdir();marker=directory/'probe-unavailable'
            actual_tmux=shutil.which('tmux');assert actual_tmux
            wrapper=tools/'tmux'
            wrapper.write_text('#!/bin/sh\nif [ -e '+shlex.quote(str(marker))+' ]; then exit 69; fi\nexec '+shlex.quote(actual_tmux)+' "$@"\n');wrapper.chmod(0o700)
            wrapper_env={**env,'PATH':str(tools)+os.pathsep+env.get('PATH','')}
            server=subprocess.Popen([str(BINARY)],env=wrapper_env,cwd=directory,stdout=log,stderr=log);token=None;token=login(server)
            # A new browser attaches a new shell. Terminate only its own tmux
            # client, while making backend probes fail; the shell must survive.
            browser=subprocess.Popen(['node','-e',NODE],cwd=ROOT/'frontend',stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
            browser.stdin.write(json.dumps({'base':f'http://127.0.0.1:{port}','token':token,'expectEOF':True}));browser.stdin.close()
            eof_id=json.loads(browser.stdout.readline())['session_id']
            clients=subprocess.run([actual_tmux,'list-clients','-F','#{client_pid}'],env=env,capture_output=True,check=True).stdout.splitlines()
            assert len(clients)==1,'Fixture has unexpected tmux clients'
            client_pid=int(clients[0]);assert client_pid>1
            marker.touch();os.kill(client_pid,signal.SIGTERM)
            closed=json.loads(browser.stdout.readline())
            assert closed=={'closed':True,'ended':False},'Unavailable probe reported an ended session'
            assert browser.wait(timeout=10)==0
            for _ in range(50):
                code,value=request('/api/terminal/sessions')
                retained=next((s for s in value['data'] if s['id']==eof_id),None)
                if retained and retained['ws_count']==0:break
                time.sleep(.1)
            assert retained and retained['state']=='unknown' and retained['ws_count']==0
            assert eof_id in json.loads(path.read_bytes()) and server.poll() is None
            marker.unlink();code,value=request('/api/terminal/sessions')
            assert next(s for s in value['data'] if s['id']==eof_id)['state']=='alive'
            print('PASS: actual attach EOF during unavailable probes preserves metadata, omits END, decrements live count and recovers the surviving shell.')
            preferences_path=directory/'auth/terminal_settings.json';reviewed_preferences=directory/'auth/reviewed-preferences.json'
            previous_preferences=preferences_path.read_bytes();preferences_path.replace(reviewed_preferences);preferences_path.mkdir()
            code,value=request('/api/terminal/settings',{**settings,'font_size':25},method='PUT')
            assert code==503 and not value['success'] and reviewed_preferences.read_bytes()==previous_preferences
            code,value=request('/api/terminal/settings');assert code==503 and not value['success']
            code,value=request('/api/terminal/sessions/'+eof_id+'/keepalive',{'keepalive':False});assert code==503 and not value['success']
            before_tmux=subprocess.run([actual_tmux,'list-sessions','-F','#{session_name}'],env=env,capture_output=True,check=True).stdout
            denied=subprocess.run(['node','-e',NODE_DENIED],cwd=ROOT/'frontend',input=json.dumps({'base':f'http://127.0.0.1:{port}','token':token,'expectText':'Terminal preferences unavailable'}),text=True,capture_output=True,timeout=30)
            assert denied.returncode==0 and server.poll() is None
            assert subprocess.run([actual_tmux,'list-sessions','-F','#{session_name}'],env=env,capture_output=True,check=True).stdout==before_tmux
            code,value=request('/api/terminal/sessions')
            assert code==200 and all(next(s for s in value['data'] if s['id']==sid)['state']=='alive' for sid in [identifier,eof_id])
            preferences_path.rmdir();reviewed_preferences.replace(preferences_path)
            code,value=request('/api/terminal/settings');assert code==503,'Live quarantine silently cleared after filesystem repair'
            server.terminate();server.wait(timeout=10)
            server=subprocess.Popen([str(BINARY)],env=env,cwd=directory,stdout=log,stderr=log);token=None;token=login(server)
            code,value=request('/api/terminal/settings');assert code==200 and value['data']==settings
            code,value=request('/api/terminal/sessions')
            assert code==200 and all(next(s for s in value['data'] if s['id']==sid)['state']=='alive' for sid in [identifier,eof_id])
            print('PASS: preferences persist across restart; failed save returns 503 and quarantines reads/new sessions without stopping existing shells; explicit reviewed recovery restores confirmed preferences.')

        finally:
            if browser and browser.poll() is None:
                os.killpg(browser.pid,signal.SIGTERM)
                try:browser.wait(timeout=5)
                except subprocess.TimeoutExpired:os.killpg(browser.pid,signal.SIGKILL);browser.wait(timeout=5)
            if server.poll() is None:
                server.terminate()
                try:server.wait(timeout=10)
                except subprocess.TimeoutExpired:server.kill();server.wait(timeout=5)
            path=directory/'auth/terminal_sessions.json'
            if path.is_file():
                for session in json.loads(path.read_text()).values():
                    name=session.get('tmux_name','')
                    if name.startswith('nb_admin_'):subprocess.run(['tmux','kill-session','-t','='+name],env=env,capture_output=True)
